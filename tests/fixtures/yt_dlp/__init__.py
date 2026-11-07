from __future__ import annotations

from types import SimpleNamespace

version = SimpleNamespace(__version__="fixture-version")


def parse_options(args):
    return SimpleNamespace(
        ydl_opts={"fixture_args": list(args)},
        urls=list(args),
    )


class YoutubeDL:
    instances = []

    def __init__(self, options):
        self.options = dict(options)
        self.instance_id = len(self.__class__.instances) + 1
        self.__class__.instances.append(self)

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_value, traceback):
        return False

    def download(self, urls):
        print(
            f"fixture-instance-{self.instance_id}:"
            f"{','.join(self.options['fixture_args'])}:"
            f"{','.join(urls)}"
        )
        return 0
