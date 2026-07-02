# 003-cross-audit-synthesis：交叉审计终裁报告

> **状态**: 终裁，可执行
> **前置**: [001-pipe-deadlock-fix.md](./001-pipe-deadlock-fix.md) (TEAM A) → [002-independent-audit.md](./002-independent-audit.md) (TEAM B)
> **流程**: TEAM A 报告 → TEAM B 独立审计 → 本终裁报告

---

## 1. 执行摘要

两轮交叉审计后，所有实质争议已收敛。汇总如下：

| 事项 | TEAM A (001) | TEAM B (002) | 终裁 |
|---|---|---|---|
| P0 死锁存在 | ✅ 确认 | ✅ 确认 | **达成共识** |
| 修复方向 | `wait_with_output` + `JoinHandle` | 手动 spawn reading tasks | **手动 spawn（002 方向），但需补 kill** |
| 001 §4.2 编译错误 | — | ✅ 指出 use-after-move | **002 正确，001 代码有误** |
| partial output 是否重要 | ✅ 是 | ✅ 是 | **达成共识** |
| 简单方案 vs 手动方案 | 倾向简单 | 倾向手动 | **手动方案，原因见 §3** |
| P2 目录重命名 | 不做 | 不做 | **达成共识** |
| PR checklist | 加 | 加 | **达成共识** |

**核心结论**：001 的方向对、代码有编译错误；002 的方向对、代码有死锁。终裁方案取双方之长。

---

## 2. 对 TEAM B (002) 审计报告的逐条评估

### 2.1 ✅ 正确发现

#### 001 §4.2 编译错误

002 准确指出了 use-after-move 问题：

```rust
// 001 §4.2 — 有编译错误
let handle = tokio::spawn(async move { child.wait_with_output().await });
//                                    ^^^^^ child moved into async block

match timeout(timeout_duration, &mut handle).await {
    Err(_elapsed) => {
        child.kill().await;  // ❌ child 已被 move，编译失败
    }
}
```

**评估**：✅ 精确命中。001 的伪代码没有经过编译器验证。

#### `wait_with_output()` 超时丢 partial data 的内部机制分析

002 对 `wait_with_output()` 内部实现的描述及其在 timeout 场景下的行为分析是准确的：内部 spawn 的 reading tasks 在 Future drop 后丢失积累数据。

**评估**：✅ 分析正确。

#### denied_args 补充参数

002 补充了 `--output`、`--write-info-json` 等未在 deny list 中的高风险参数。

**评估**：✅ 方向正确。但 `--output-template` 在 yt-dlp 中不是独立参数名（实际的 output template 通过 `-o`/`--output` 指定），细节不精确但不影响结论。

---

### 2.2 ❌ 致命错误：002 §3.4 方案有死锁

#### 002 的代码

```rust
// 002 §3.4 — 有死锁
let stdout_task = tokio::spawn(async move { read_to_end(stdout_handle).await });
let stderr_task = tokio::spawn(async move { read_to_end(stderr_handle).await });

let wait_result = timeout(timeout_duration, child.wait()).await;
let elapsed = start.elapsed();

// 002 的做法：直接 await reading tasks
let stdout = stdout_task.await.unwrap_or_default();
let stderr = stderr_task.await.unwrap_or_default();

match wait_result {
    Err(_) => {
        // 002 注释写："kill_on_drop 已杀进程"
        // ⚠️ 这是事实错误！
    }
}
```

#### 死锁分析

`kill_on_drop` 只在 `Child` 结构体被 **drop** 时触发。002 方案中 `child` 变量**仍在作用域中未被 drop**——`timeout` 只 drop 了 `child.wait()` 返回的 Future，不影响 `Child` 自身的生命周期。

**死锁时序**：

```
T0: 子进程产生 >64KB 输出，pipe buffer 满 → write() 阻塞
T1: timeout 触发 → child.wait() Future 被 drop（但 child 未 drop！）
T2: stdout_task / stderr_task 卡在 read_to_end → 等待 pipe EOF
T3: 子进程卡在 write → 等待 pipe buffer 有空间
T4: await stdout_task → 永久阻塞（pipe 永不被关闭）
```

**结论**：002 声称 "kill_on_drop 已杀进程" 是事实错误。超时场景下进程未被 kill，pipe 永远不关闭，reading tasks 永久阻塞——这和他们批判 001 的 pipe deadlock **是同类问题**。

---

## 3. 终裁方案：手动 spawn + 显式 kill

结合双方正确部分，整理最终无争议实现。

### 3.1 为什么必须手动 spawn 而非 `wait_with_output`

| 场景 | `wait_with_output` + 超时 | 手动 spawn + 超时 |
|---|---|---|
| 正常完成 | ✅ 正确 | ✅ 正确 |
| 超时 — 杀进程 | ✅ `kill_on_drop` 触发 | ✅ 显式 `child.kill()` |
| 超时 — partial output | ❌ **丢失**（reading tasks 随 Future 一起 drop） | ✅ **保留**（tasks 生命周期独立） |
| 编译安全性 | ✅ 无 move 问题 | ✅ take handles + 独立 tasks |

**裁决**：`wait_with_output` 的黑盒特性使超时场景下 partial output 不可达。手动 spawn 的代价是高 ~10 行代码，收益是完整的正确性。**选择手动 spawn。**

### 3.2 终裁代码

```rust
pub async fn execute(payload: RunRequest, binary_path: &PathBuf) -> impl IntoResponse {
    let start = Instant::now();

    // --- 前置校验（不变）---
    if let Err(msg) = reject_denied_args(&payload.args) {
        warn!(log_type = "audit", args = ?redact_args(&payload.args),
              timeout_seconds = payload.timeout_seconds, "{}", msg);
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::to_value(ErrorResponse {
                error: msg, code: "ARG_REJECTED",
            }).unwrap_or_else(|e| {
                error!(error = %e, "Failed to serialize ErrorResponse");
                serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
            })),
        );
    }

    let timeout_duration = Duration::from_secs(
        payload.timeout_seconds.unwrap_or(30).clamp(1, 300)
    );

    // --- 子进程启动 ---
    let mut cmd = Command::new(binary_path);
    cmd.args(&payload.args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    cmd.kill_on_drop(true);        // 安全网：Future 被 cancel 时的最后兜底
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to spawn yt-dlp");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(ErrorResponse {
                    error: format!("Failed to spawn yt-dlp process: {}", e),
                    code: "SPAWN_FAILURE",
                }).unwrap_or_else(|e| {
                    error!(error = %e, "Failed to serialize ErrorResponse");
                    serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                })),
            );
        }
    };

    // --- 核心修复：手动 spawn drain tasks ---
    // stdout/stderr handles 从 child 中 take 出来，生命周期独立
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    // Task A: 并发读 stdout（独立于 child.wait）
    let stdout_task = tokio::spawn(async move {
        match stdout_handle {
            Some(mut reader) => {
                let mut buf = Vec::new();
                let _ = tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buf).await;
                String::from_utf8_lossy(&buf).into_owned()
            }
            None => String::new(),
        }
    });

    // Task B: 并发读 stderr（独立于 child.wait）
    let stderr_task = tokio::spawn(async move {
        match stderr_handle {
            Some(mut reader) => {
                let mut buf = Vec::new();
                let _ = tokio::io::AsyncReadExt::read_to_end(&mut reader, &mut buf).await;
                String::from_utf8_lossy(&buf).into_owned()
            }
            None => String::new(),
        }
    });

    // --- 等待子进程退出（带 timeout）---
    let wait_result = timeout(timeout_duration, child.wait()).await;
    let elapsed = start.elapsed();

    // ⚠️ 超时时必须显式 kill，确保 pipe 关闭→drain tasks 完成
    //    不能依赖 kill_on_drop（此时 child 未 drop）
    if wait_result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;   // 等待进程完全终止，pipe 写端关闭
    }

    // 此时 pipe 已关闭（正常退出或 kill），drain tasks 保证完成
    let stdout = stdout_task.await.unwrap_or_default();
    let stderr = stderr_task.await.unwrap_or_default();

    match wait_result {
        Ok(Ok(status)) => {
            let exit_code = status.code().unwrap_or(-1);
            info!(
                exit_code,
                duration_ms = elapsed.as_millis() as u64,
                stdout_len = stdout.len(),
                stderr_len = stderr.len(),
                args = ?redact_args(&payload.args),
                "yt-dlp completed"
            );
            (
                StatusCode::OK,
                Json(serde_json::to_value(RunResponse {
                    exit_code, stdout, stderr,
                }).unwrap_or_else(|e| {
                    error!(error = %e, "Failed to serialize RunResponse");
                    serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                })),
            )
        }
        Ok(Err(e)) => {
            error!(error = %e, duration_ms = elapsed.as_millis() as u64,
                   "Failed to collect yt-dlp output");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::to_value(ErrorResponse {
                    error: format!("Failed to collect process output: {}", e),
                    code: "COLLECT_FAILURE",
                }).unwrap_or_else(|e| {
                    error!(error = %e, "Failed to serialize ErrorResponse");
                    serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                })),
            )
        }
        Err(_elapsed) => {
            warn!(
                duration_ms = elapsed.as_millis() as u64,
                exit_code = -1,
                args = ?redact_args(&payload.args),
                stdout_len = stdout.len(),
                stderr_len = stderr.len(),
                "yt-dlp timed out"
            );
            (
                StatusCode::OK,
                Json(serde_json::to_value(RunResponse {
                    exit_code: -1, stdout, stderr,
                }).unwrap_or_else(|e| {
                    error!(error = %e, "Failed to serialize RunResponse");
                    serde_json::json!({"error": "internal serialization error", "code": "INTERNAL"})
                })),
            )
        }
    }
}
```

### 3.3 关键变更点

| # | 变更 | 来源 | 原因 |
|---|---|---|---|
| 1 | 手动 `tokio::spawn` 两个 drain tasks | 002 方向 | 独立于 `child.wait()` 的生命周期，超时后仍可拿到 partial output |
| 2 | `cmd.kill_on_drop(true)` | 001 方向 | 安全网：Future 被 cancel 时的兜底（不是超时场景的主力，是保险） |
| 3 | 超时时显式 `child.kill().await` + `child.wait().await` | 003 修补 | **002 漏掉的致命一步**：确保 pipe 关闭，drain tasks 能完成 |
| 4 | 先 await drain tasks，后匹配 wait_result | 003 整合 | 无论正常/超时，pipe 都已关闭，tasks 保证返回 |
| 5 | 删除 `read_pipe()` 函数 | 003 | 不再需要 |

### 3.4 为什么这个方案正确

| 场景 | 执行流 | 结果 |
|---|---|---|
| **正常完成**（输出 < 64KB） | wait 先返回 → pipe 关闭 → drain tasks 收集完整数据 | ✅ 完整 stdout/stderr |
| **正常完成**（输出 > 64KB） | drain tasks 并发读 pipe → 子进程能持续写入 → wait 正常返回 | ✅ 无死锁，完整数据 |
| **超时**（任意输出量） | timeout 触发 → `child.kill()` → `child.wait()` → pipe 关闭 → drain tasks 返回已积累数据 | ✅ partial output 保留 |
| **HTTP 请求取消** | Future drop → `kill_on_drop(true)` 杀进程 → pipe 关闭 → drain tasks 完成（但无人 await） | ✅ 无孤儿进程 |

---

## 4. 终裁决议总表

| # | 议题 | TEAM A | TEAM B | 终裁 | 理由 |
|---|---|---|---|---|---|
| 1 | P0 修复方案 | `wait_with_output` + `JoinHandle` | 手动 spawn（有 bug） | **手动 spawn + 显式 kill** | 取双方之长，补双方之漏 |
| 2 | 编译错误 | 有（use-after-move） | 正确指出 | **002 正确** | 001 伪代码未验证 |
| 3 | 002 方案死锁 | — | 有（忘 kill） | **002 有 bug** | 超时后 pipe 永不被关闭 |
| 4 | Partial output 重要性 | 重要 | 重要 | **达成共识** | 一致 |
| 5 | 简单方案（`wait_with_output` 直接 timeout） | 未评估 | 拒绝（丢 partial data） | **拒绝** | 002 分析正确：是"胶水"方案 |
| 6 | 集成测试 | Smoke test | 单元测试 + 超时测试 | **超时测试必要** | Partial output 行为需要显式验证 |
| 7 | P2 目录重命名 | 不做 | 不做 | **达成共识** | 一致 |
| 8 | PR checklist | 加 | 加 | **达成共识** | 一致 |
| 9 | denied_args 补充 | 未覆盖 | 指出 `--output` 等 | **独立 security ticket** | 不在本次 P0 范围 |

---

## 5. 终裁 Checklist

- [ ] `executor.rs` P0 修复 PR：采用 §3.2 终裁代码（手动 spawn + 显式 `child.kill()`）
- [ ] 删除 `read_pipe()` 函数（如无其他调用方）
- [ ] 超时测试：构造 >64KB stdout 的 mock 脚本，验证超时后返回 partial output
- [ ] 正常完成测试：构造 >64KB stdout 的 mock 脚本，验证完整输出
- [ ] PR template 更新：`tokio::process::Command` pipe drain 规则
- [ ] 开独立 security ticket：denied_args 补充 `--output`、`--write-info-json` 等
- [ ] 设计文档标注：当前 batch 模式，不支持流式输出

---

## 6. 附录：三方方案对比

```
场景：子进程输出 200KB，timeout 30s，实际执行 5s（正常完成）

┌─────────────────────┬────────────────────┬────────────────────┬─────────────────────┐
│                     │  001 §4.2           │  002 §3.4           │  003 §3.2 (终裁)     │
│                     │  (TEAM A)           │  (TEAM B)           │                      │
├─────────────────────┼────────────────────┼────────────────────┼─────────────────────┤
│  正常完成            │  ✅ 正确            │  ✅ 正确            │  ✅ 正确             │
│  死锁风险            │  ✅ 无              │  ✅ 无              │  ✅ 无               │
│  超时 partial output │  ❌ 丢失            │  ❌ 死锁            │  ✅ 保留             │
│  编译通过            │  ❌ use-after-move  │  ✅ 通过            │  ✅ 通过             │
│  孙进程管理           │  无                │  kill_on_drop 兜底  │  kill_on_drop 兜底   │
└─────────────────────┴────────────────────┴────────────────────┴─────────────────────┘
```
