# Architecture

src/

components/
pages/
hooks/
services/
store/
types/

src-tauri/

providers/
- wallhaven.rs
- pexels.rs

wallpaper/
- apply.rs
- scheduler.rs

database/
- sqlite.rs

cache/
- downloader.rs

commands/
- search.rs
- collections.rs
- favorites.rs

## Design Rules

- Provider architecture must be extensible.
- New providers can be added without changing existing providers.
- Frontend must communicate through Tauri commands.
- Business logic must remain in Rust.
