import { defineConfig } from 'vitest/config';

// Only this checkout's sources: agent worktrees, build output and Rust targets hold copies of the same tests.
export default defineConfig({test:{include:['src/**/*.test.{ts,tsx}'],exclude:['**/node_modules/**','**/dist/**','**/target/**','.claude/**']}});
