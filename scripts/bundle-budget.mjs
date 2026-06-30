import { existsSync, globSync, readFileSync } from 'node:fs'
import { gzipSync } from 'node:zlib'

// Frontend gzip bundle ceiling (90 KiB). Regression tripwire for accidental bloat.
const CEILING_BYTES = 92160

if (!existsSync('build')) {
  console.error('build/ not found. Run `pnpm build` first.')
  process.exit(1)
}

const files = [...globSync('build/**/*.js'), ...globSync('build/**/*.css')]
const entries = files
  .map((file) => ({ file, bytes: gzipSync(readFileSync(file)).length }))
  .sort((a, b) => b.bytes - a.bytes)

const total = entries.reduce((sum, entry) => sum + entry.bytes, 0)
const kib = (bytes) => (bytes / 1024).toFixed(2)

console.log('gzip bundle sizes:')
for (const { file, bytes } of entries) {
  console.log(
    `  ${String(bytes).padStart(7)} B  ${kib(bytes).padStart(7)} KiB  ${file}`,
  )
}
console.log(
  `  ${String(total).padStart(7)} B  ${kib(total).padStart(7)} KiB  total`,
)

if (total > CEILING_BYTES) {
  console.error(
    `Bundle budget exceeded: ${total} B (${kib(total)} KiB) > ${CEILING_BYTES} B (${kib(CEILING_BYTES)} KiB).`,
  )
  process.exit(1)
}

console.log(
  `Within budget: ${total} B (${kib(total)} KiB) <= ${CEILING_BYTES} B (${kib(CEILING_BYTES)} KiB).`,
)
