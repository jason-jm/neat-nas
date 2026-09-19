# Marketing assets

Each design comes in a light and a dark variant; the `.svg` is the source, the `.png` a render.

| File | Size | Use |
| --- | --- | --- |
| `social-preview-{light,dark}` | 1280×640 | GitHub social preview (Settings → Social preview), X/Twitter cards, Product Hunt gallery |
| `hero-{light,dark}` | 1600×1000 | Blog posts, forum threads, the first slide of a video |
| `three-things-{light,dark}` | 1600×800 | The "address / user name / password" pitch as one image |

Re-render after editing an SVG: `qlmanage -t -s 1600 -o . hero-light.svg && mv hero-light.svg.png hero-light.png`.
Real screenshots of the app in both themes belong next to these; see the launch kit for the shot list.
