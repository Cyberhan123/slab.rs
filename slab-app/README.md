# Tauri + React + Typescript

This template should help get you started developing with Tauri, React and Typescript in Vite.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Utility Scripts

Convert Figma color values into OKLCH CSS tokens:

```sh
bun run color:oklch -- background=#f7f9fb primary=#0d9488
bun run color:oklch -- "--surface-soft: #f2f4f6;" "--user-bubble: #d5e3fd;"
```

Read a batch from stdin:

```sh
echo "--background: #f7f9fb;" | bun run color:oklch -- --stdin
```

Convert a CSS file in place or to a second file:

```sh
bun run color:oklch -- --file src/styles/globals.css --out src/styles/globals.oklch.css
bun run color:oklch -- --file src/styles/globals.css --write --annotate
```
