import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { execFileSync } from 'node:child_process'
import { readFileSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
const root = fileURLToPath(new URL('../..', import.meta.url))
const git = (args: string[]) => { try { return execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim() } catch { return null } }
const source = existsSync(root + '/.git') ? git(['rev-parse', 'HEAD']) : null
const dirty = !source || git(['status', '--porcelain', '--untracked-files=normal']) !== ''
const channel = process.env.SKILLTAPE_BUILD_CHANNEL ?? 'source'
if (!['source', 'release'].includes(channel)) throw new Error('Invalid build channel')
if (channel === 'release' && (!process.env.GITHUB_REF?.startsWith('refs/tags/v') || dirty || git(['rev-parse', `${process.env.GITHUB_REF}^{commit}`]) !== source)) throw new Error('Release source must be a clean immutable tag')
const catalog = JSON.parse(readFileSync(root + '/product-capabilities.json', 'utf8'))
const metadata = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'))
const identity = { schema: 'skilltape.build-info/v1', version: metadata.version, sourceRevision: source, dirty, channel, capabilities: catalog.capabilities }
export default defineConfig({
  plugins: [react(), { name: 'skilltape-build-identity', generateBundle() { this.emitFile({ type: 'asset', fileName: 'build-info.json', source: JSON.stringify(identity) + '\n' }) } }],
  define: { __SKILLTAPE_BUILD__: JSON.stringify(identity) },
  server: { host: '127.0.0.1', port: 4173 },
})
