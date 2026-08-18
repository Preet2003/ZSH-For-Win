# WinZSH website

Static marketing page for SEO and downloads.

## Live URL

After Pages is enabled and this workflow runs:

**https://preet2003.github.io/ZSH-For-Win/**

Download button:

`https://github.com/Preet2003/ZSH-For-Win/releases/latest/download/WinZSH-Setup-x86_64.exe`

## Deploy

Workflow: `.github/workflows/pages.yml`

1. Push `website/` + the workflow to `main`.
2. Repo **Settings → Pages → Build and deployment → Source: GitHub Actions**.
3. Run **Actions → Pages → Run workflow** (or push a change under `website/`).
4. Open the Pages URL and test Download.

## Local preview

```powershell
cd website
# open index.html in a browser, or:
npx --yes serve .
```
