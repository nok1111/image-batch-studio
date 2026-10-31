# ADR 0001 — Tauri 2 + React/TypeScript + Rust

Status: accepted

**Context.** Commercial Windows-first desktop app, macOS/Linux later, small installer, fast startup.

**Decision.** Tauri 2 shell, React + TypeScript (Vite) UI, Tailwind CSS + shadcn/ui-style components (Radix primitives, copied into `src/components/ui`).

**Consequences.** ~10 MB installers using the system WebView2 (bundled bootstrapper). One Rust codebase for all platforms. UI testable with Vitest/jsdom.
