# Adapted from https://github.com/jik876/hifi-gan under the MIT license.
#   LICENSE is in incl_licenses directory.


class AttrDict(dict[str, bool]):
    @property
    def snake_logscale(self) -> bool:
        return self["snake_logscale"]
