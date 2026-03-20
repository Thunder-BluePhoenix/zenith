# Quick Start

Create a `.zenith.yml` in your project root:

```yaml
version: "2"

jobs:
  build:
    runs-on: alpine
    steps:
      - name: Hello
        run: echo "Hello from Zenith!"
```

Run it:

```
zenith run
```

## With caching

```yaml
version: "2"

jobs:
  build:
    runs-on: alpine
    steps:
      - name: Install deps
        run: npm install
        watch: [package.json, package-lock.json]
        outputs: [node_modules/]

      - name: Build
        run: npm run build
        depends_on: [Install deps]
        watch: [src/**/*.ts]
        outputs: [dist/]
```

The second run of `zenith run` will restore `node_modules/` and `dist/` from cache — no re-install, no re-build.
