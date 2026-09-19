# CI workflows (parked)

`workflows/ci.yml` and `workflows/release.yml` are the GitHub Actions
pipelines described in [RELEASE.md](../RELEASE.md). They live here instead of
`.github/workflows/` because pushing workflow files needs a GitHub token with
the `workflow` scope, which the token used to publish this repository did not
have. To activate them:

```bash
gh auth refresh -h github.com -s workflow
git mv ci/workflows .github/workflows
git commit -m "Enable CI and release workflows"
git push
```

Until then, releases are built locally (see RELEASE.md, "Building locally")
and uploaded with `gh release`.
