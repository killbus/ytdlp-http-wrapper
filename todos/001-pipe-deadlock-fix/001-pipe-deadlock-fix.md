# 001-pipe-deadlock-fix：ytdlp-http-wrapper 审计响应与修复计划

> **状态**: ✅ 已审计。§4.2 代码有编译错误（use-after-move），已被 [003 终裁方案](./003-cross-audit-synthesis.md#32-终裁代码) 取代
> **来源**: Team B 审计报告 [AUDIT_REPORT.md](../AUDIT_REPORT.md)
> **流程**: Ground Fact Check → 专家聊天室讨论 → 本计划文档 → [TEAM B 交叉审计 (002)](./002-independent-audit.md) → [终裁 (003)](./003-cross-audit-synthesis.md)

---

## 1. Ground Fact Check 结果

对 Team B 审计报告的两项发现进行了逐行代码核实。

### 1.1 Finding 1：OS Pipe Buffer Saturation Deadlock（P0）— ✅ CONFIRMED

| 检查项 | 结果 | 证据 |
|---|---|---|
| 死锁机制存在 | ✅ 确认 | [executor.rs:149](../src/executor.rs#L149) — `child.wait()` 在第149行；pipe 读取在 [executor.rs:154-161](../src/executor.rs#L154-L161)，位于进程退出之后 |
| `wait()` 期间不读 pipe | ✅ 确认 | [executor.rs:146-149](../src/executor.rs#L146-L149)：先 `take()` stdout/stderr handle，然后 `await child.wait()`，最后才 `read_pipe()` |
| OS pipe buffer 限制 | ✅ 确认 | Linux 默认 64KB (`PIPE_BUF`)，Windows 类似。超过此阈值写端阻塞 |
| 超时分支处理 | ⚠️ 部分正确 | 超时后 kill + 读残留 pipe 的逻辑是好的（[executor.rs:208-241](../src/executor.rs#L208-L241)），但 `kill_on_drop(true)` 缺失 |

**审计报告遗漏的点**：
- 未提及超时场景下 `wait_with_output()` 的 partial output 丢失问题（详见 David 的发言）
- 未提及孙进程（ffmpeg）孤儿问题（详见 David 的发言）

**结论**：P0 定性准确，死锁机制描述精确。推荐方案 `wait_with_output()` 方向正确，但需要增强以保留超时 partial output。

### 1.2 Finding 2：目录拼写错误（P2）— ✅ CONFIRMED

| 检查项 | 结果 | 证据 |
|---|---|---|
| 目录名缺 r | ✅ 确认 | 目录 `ytdlp-http-wapper`，应为 `ytdlp-http-wrapper` |
| Cargo.toml 名称正确 | ✅ 确认 | [Cargo.toml:2](../Cargo.toml#L2) — `name = "ytdlp-http-wrapper"` |
| 是否影响编译 | ✅ 不影响 | Cargo 使用 `Cargo.toml` 的 `name` 字段，不依赖目录名 |
| 其他代码引用 | ✅ 无影响 | 全仓搜索，无文件引用 `ytdlp-http-wapper` 路径 |

**结论**：事实正确，纯 cosmetic 问题。

---

## 2. 专家聊天室讨论记录

经过 ground fact check 确认后，组织了三位领域专家进行定向讨论。

### 2.1 David（Rust 系统编程专家）

**核心观点**：

1. **`wait_with_output()` vs 手动 spawn**：`wait_with_output()` 内部就是 spawn 读 pipe 的 task，对 yt-dlp 的典型场景（提取 metadata、列格式，通常 < 100KB）足够。但有人传 `-o -` 把视频 dump 到 stdout 时有 OOM 风险。手动 spawn 可以加 `take(N)` 限流。

2. **超时 + partial output 的关键盲区**：如果 `wait_with_output()` 被 `timeout()` 包裹后超时，child 随 future 一起被 drop，内部积累的 partial data 全丢了。

   **推荐方案**：不要把 child move 进 timeout future，而是 spawn 一个 `tokio::task`，拿 `JoinHandle`：
   ```rust
   let mut child = cmd.spawn()?;
   let handle = tokio::spawn(async move { child.wait_with_output().await });

   match timeout(dur, &mut handle).await {
       Ok(Ok(output)) => { /* 正常返回 */ }
       _ => {
           child.kill().await;      // 杀进程 → pipe 关闭 → 读 task 自然完成
           let partial = handle.await??; // 拿到残留数据
       }
   }
   ```

3. **Windows `kill_on_drop` 与孙进程**：`kill_on_drop(true)` 在 Windows 调用 `TerminateProcess`，没问题。但 yt-dlp 常 spawn ffmpeg 做后处理，kill 后 ffmpeg 变成孤儿。属于 **P1 防御**，不阻塞 P0 修复。

### 2.2 Sarah（SRE / DevOps 专家）

**核心观点**：

1. **触发概率比纸面高**：
   - `--dump-json` 对中等 playlist（50+ 视频）轻松 >200KB
   - `--list-formats` 在 playlist 模式下累积很快
   - ffmpeg 后处理的 stderr（尤其 `--verbose`）经常爆 buffer
   - yt-dlp 内部 stack trace 也会触发
   - **结论：对 playlist 操作几乎必现死锁**

2. **修复紧迫度：立即修，别等**：`wait_with_output()` + `kill_on_drop(true)` 是单函数替换，`cargo test` 跑通就能上线。临时缓解：axum 中间件限制并发 `/run` 请求数 + 默认 timeout 降到 15s。

3. **P2 目录重命名：爆炸半径几乎为零**：无文件引用错误路径。`git mv` 一下，10 秒的事，值得顺手修。

4. **集成测试建议**：在 CI 加一个 test，用脚本生成 128KB+ stdout 作为 mock yt-dlp，验证 `/run` 返回完整。

### 2.3 Maria（Technical Lead）

**核心观点**：

1. **P0：立即修，不等 sprint**：改动十行。回归测试用 `--print-to-file` 或 dump 长 playlist info-json 构造 >64KB 输出即可，不需要专门的集成测试框架。花半天修掉。

2. **测试策略：不写专门复现测试**：`wait_with_output` 语义由 Tokio 保证。加一个 smoke test 跑真实下载 URL 比 mock pipe buffer 的单元测试更可靠。

3. **P2 重命名：不做**：收益为零，风险存在（CI 脚本、本地 checkout 路径、团队成员 shell history）。不值得。

4. **流程改进**：PR template 加一条硬性规则——「所有 `tokio::process::Command` 输出使用 `wait_with_output` 或在 `wait()` 前并发 drain pipe」。这类死锁在 async Rust 里重复出现了太多次。

---

## 3. 判官总结

### 3.1 讨论质量评价

| 维度 | 评价 |
|---|---|
| 真洞察 | David 的 `JoinHandle` 方案补了审计报告遗漏的超时 partial output 盲区；Sarah 的具体触发场景把风险从理论拉到现实 |
| 有交锋 | Sarah vs Maria 在测试策略（mock vs smoke）和 P2 重命名（修 vs 不动）上有实质性分歧 |

### 3.2 补盲区

三位专家均未提及：

1. **流式输出场景**：如果未来需要支持实时进度输出，`wait_with_output()` 是 batch 模式，不支持流式。David 提到的"手动 spawn drain pipe"反而是流式场景的正确架构。**现在不需要改，但应在设计文档标注约束。**

2. **stderr 分离的价值与架构选择**：当前 `RunResponse` 将 stdout 和 stderr 分开返回（[models.rs:11-13](../src/models.rs#L11-L13)），这是好设计——调用方可以独立处理正常输出和错误日志。如果未来做实时流式推送（如 WebSocket 推送 error 日志），需要按流区分——David 的"手动 spawn 两个 task 分别 drain pipe"架构天然支持按流分发；`wait_with_output()` 虽然 batch，但内部已分开收集到 `output.stdout` 和 `output.stderr`，同样保留了这个分离。**无论选哪种方案，stdout/stderr 分离的设计资产都应保留。**

3. **denied_args 安全面**：[executor.rs:14-29](../src/executor.rs#L14-L29) 中 `--exec` 被 deny 但 `--output` 未限制。用户传 `--output naughty_template` 可能写到任意路径。虽然不在本次审计范围，但值得单独关注。

### 3.3 执行建议

1. **立即（P0）**：用 `wait_with_output()` + `kill_on_drop(true)` 修死锁，采用 David 的 `JoinHandle` 变体保留超时 partial output。改动 ~15 行，半天工作量。
2. **本周**：PR template 加 review checklist。
3. **P2 重命名**：随缘。下次改 CI/CD 时顺手 `git mv`。不值得单独 PR。

---

## 4. 修复方案（供 Team B 交叉审计）

### 4.1 当前代码问题

```rust
// executor.rs:146-161 — 当前有缺陷的实现
let stdout_handle = child.stdout.take();
let stderr_handle = child.stderr.take();

let result = timeout(timeout_duration, child.wait()).await; // ← 不读 pipe！

match result {
    Ok(Ok(status)) => {
        // 进程退出了才读 pipe — 如果输出 > 64KB，永远不会到这里
        let stdout = read_pipe(stdout_handle).await;
        let stderr = read_pipe(stderr_handle).await;
        // ...
    }
    // ...
}
```

**死锁时序**：
```
Subprocess: write("big output") → pipe buffer 满 → write() 阻塞
Parent:     child.wait()         → 等 subprocess 退出   → wait() 阻塞
结果:       互相等待 → 永久死锁（直到 HTTP timeout）
```

### 4.2 推荐修复

> ⚠️ **交叉审计注**：此代码有编译错误（`child` use-after-move），且 `wait_with_output` 超时时丢失 partial output。
> 经 TEAM B 审计 + 003 终裁，最终方案改为 [手动 spawn + 显式 kill](./003-cross-audit-synthesis.md#32-终裁代码)。
> 保留此节原始内容供审计追溯。

<details>
<summary>原始代码（有编译错误，已被 003 取代）</summary>

```rust
use tokio::process::Command;
use std::process::Stdio;

pub async fn execute(payload: RunRequest, binary_path: &PathBuf) -> impl IntoResponse {
    // ... 前置校验代码不变 ...

    let mut cmd = Command::new(binary_path);
    cmd.args(&payload.args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);       // ← 新增：确保 Future drop 时杀子进程
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let mut child = cmd.spawn() /* ... error handling ... */;

    // 修复核心：spawn 到独立 task，超时时仍可拿 partial output
    let handle = tokio::spawn(async move { child.wait_with_output().await });
    //                                 ^^^^^ child moved here — line 190 不能再访问

    match timeout(timeout_duration, &mut handle).await {
        Ok(Ok(output)) => {
            // 正常完成：拿到完整 stdout/stderr
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            let exit_code = output.status.code().unwrap_or(-1);
            // ... 返回成功响应 ...
        }
        Ok(Err(e)) => {
            // spawn 或 wait_with_output 自身失败
            // ... 返回错误响应 ...
        }
        Err(_elapsed) => {
            // 超时：先杀进程，再等 drain task 完成拿 partial output
            child.kill().await;    // ❌ 编译错误：child 已被 move 到 async block
            let partial = handle.await
                .unwrap_or(Err(std::io::Error::new(std::io::ErrorKind::Other, "join failed")));

            let (stdout, stderr) = match partial {
                Ok(output) => (
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ),
                Err(_) => (String::new(), "Timeout".into()),
            };
            // ... 返回超时响应（含 partial output）...
        }
    }
}
```

**两个问题**：
1. ❌ **编译错误**（line 192）：`child` 已被 move 进 `async move` block（line 176），不能再调用 `child.kill()`
2. ⚠️ **超时丢 partial data**（分析见 [003 §3.1](./003-cross-audit-synthesis.md#31-为什么必须手动-spawn-而非-wait_with_output)）：`wait_with_output()` 内部 spawn 的 tasks 在 Future drop 后不可达

</details>

### 4.3 关键变更点

| # | 变更 | 原因 |
|---|---|---|
| 1 | `cmd.kill_on_drop(true)` | HTTP 请求取消时自动杀子进程，防止孤儿进程 |
| 2 | `child.wait_with_output()` 替换 `child.wait()` | Tokio 内部并发 drain pipe，消除死锁 |
| 3 | `tokio::spawn` + `JoinHandle` | 超时后仍能拿 partial output（David 方案） |
| 4 | 删除 `read_pipe()` 函数（如无其他调用） | `wait_with_output()` 内部已处理 |

### 4.4 不在此次修复范围的事项

- **孙进程管理**（ffmpeg 孤儿）：P1，需要 job object / process group，留待后续
- **流式输出支持**：当前需求不涉及，标注架构约束即可
- **denied_args 安全面强化**：独立的安全审计议题

---

## 5. 决策裁决

专家在以下议题上有分歧，由 Tech Lead（Maria）裁决：

| 议题 | Sarah | Maria | 裁决 | 理由 |
|---|---|---|---|---|
| 集成测试策略 | CI mock pipe buffer 测试 | Smoke test 即可 | **Smoke test** | `wait_with_output` 正确性由 Tokio 保证；mock 测试维护成本高于收益 |
| P2 目录重命名 | `git mv`，10秒修掉 | 不做 | **不做** | 收益为零；尽管爆炸半径小，但无理由改动 |
| PR checklist | 未提及 | 加硬性规则 | **加 checklist** | 低成本防回归，无争议 |

---

## 6. Checklist 交付物

- [ ] `executor.rs` P0 修复 PR（`wait_with_output` + `kill_on_drop` + `JoinHandle`）
- [ ] Smoke test：真实 yt-dlp 调用 `--dump-json` 验证 >64KB 输出不超时
- [ ] PR template 更新：添加 `tokio::process::Command` pipe drain 规则
- [ ] 设计文档标注：当前不支持流式输出，`wait_with_output` 为 batch 模式
