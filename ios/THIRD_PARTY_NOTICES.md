# iOS third-party notices

The iOS target embeds CPython and yt-dlp for in-process media extraction.

## Keraunos-derived local extraction bridge

The Python extraction bridge, JavaScriptCore adapter, and embedded-package layout
are adapted from [Keraunos](https://github.com/LiLiKazine/Keraunos), commit
`863a436cfc3028f38ff8e1f940a66a3e68c53104` (2026-08-17).

Keraunos is licensed under GNU GPL version 3. A copy is stored at
`ThirdParty/Keraunos/LICENSE`. If this iOS application is distributed to another
person, the corresponding iOS source and GPL notices must be provided under GPLv3.
This notice does not relicense the separately built macOS or Windows applications.

## Other embedded components

- CPython / BeeWare Python-Apple-support 3.13-b14: Python Software Foundation license.
- yt-dlp 2026.06.09: The Unlicense.
- yt-dlp-ejs 0.8.0: The Unlicense.

These components run entirely inside the application process. No remote extraction
service is contacted by YtbDown; the source websites are contacted directly.
