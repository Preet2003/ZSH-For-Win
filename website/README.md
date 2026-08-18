# WinZSH website

Static marketing page for SEO and downloads.

## Live URL

**https://preet2003.github.io/ZSH-For-Win/**

## Enable Pages (fixes deploy 404)

The workflow fails with `Failed to create deployment (status: 404)` until you do this once:

1. Open https://github.com/Preet2003/ZSH-For-Win/settings/pages
2. Under **Build and deployment → Source**, choose **GitHub Actions**
3. Save
4. Actions → **Pages** → **Run workflow**

Node 20 deprecation messages from `deploy-pages` are warnings only; ignore them.

## Download button

`https://github.com/Preet2003/ZSH-For-Win/releases/latest/download/WinZSH-Setup-x86_64.exe`

Requires a published Release that attaches `WinZSH-Setup-x86_64.exe`.
