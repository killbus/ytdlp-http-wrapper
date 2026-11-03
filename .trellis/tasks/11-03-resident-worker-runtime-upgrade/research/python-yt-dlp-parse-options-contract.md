# Python yt-dlp `parse_options` contract

## Scope

This note records the source-level verification used by the resident Python
worker. It is not a runtime validation against an installed production
generation.

## Reference

- Upstream repository: `https://github.com/yt-dlp/yt-dlp`
- Reference clone location:
  `C:\Users\ying\AppData\Local\Temp\yt-dlp-python-reference`
- Reference revision: `b77c9304e1c4220cca450c583491904daf8455af`
- Clone policy: depth-1, outside the repository, read-only reference only

## Verified contract

At the reference revision, upstream declares:

```python
ParsedOptions = collections.namedtuple(
    'ParsedOptions',
    ('parser', 'options', 'urls', 'ydl_opts'),
)
```

The `parse_options(argv=None)` documentation also states that it returns
`ParsedOptions(parser, opts, urls, ydl_opts)`.

Therefore the worker's current adapter is aligned with the upstream Python
API:

```python
parsed = yt_dlp.parse_options(list(args))
options = dict(parsed.ydl_opts)
urls = list(parsed.urls)
```

The worker creates a new `YoutubeDL(options)` instance for every request. It
does not reuse the parsed options mapping or downloader instance.

## Validation boundary

This verification establishes the source contract only. The local machine did
not have the real `yt_dlp` Python package installed, and no dependency was
installed for this check. Runtime-generation validation must still import and
exercise the exact packaged version before that generation can be promoted.

