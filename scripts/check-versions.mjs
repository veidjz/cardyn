import { readFileSync } from 'node:fs'

// All three release manifests must share one version. A release tag publishes
// all of them, so a forgotten bump silently ships a mismatched release.

const sources = []

const pkg = JSON.parse(readFileSync('package.json', 'utf8'))
sources.push({ file: 'package.json', version: pkg.version })

const tauriConf = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8'))
sources.push({ file: 'src-tauri/tauri.conf.json', version: tauriConf.version })

// Cargo.toml is TOML, not JSON. Scope the lookup to the [package] section so we
// never match a version under [dependencies] / [build-dependencies].
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8')
const lines = cargo.split('\n')
const packageStart = lines.findIndex((line) => line.trim() === '[package]')
let cargoVersion
if (packageStart !== -1) {
  const rest = lines.slice(packageStart + 1)
  const nextSection = rest.findIndex((line) => /^\s*\[/.test(line))
  const sectionLines = nextSection === -1 ? rest : rest.slice(0, nextSection)
  const match = sectionLines.join('\n').match(/^version\s*=\s*"([^"]+)"/m)
  cargoVersion = match ? match[1] : undefined
}
sources.push({ file: 'src-tauri/Cargo.toml', version: cargoVersion })

const width = Math.max(...sources.map((source) => source.file.length))
for (const { file, version } of sources) {
  console.log(`  ${file.padEnd(width)}  ${version ?? '(missing)'}`)
}

const missing = sources.filter((source) => !source.version)
if (missing.length > 0) {
  console.error(
    `Missing version in: ${missing.map((source) => source.file).join(', ')}.`,
  )
  process.exit(1)
}

const versions = new Set(sources.map((source) => source.version))
if (versions.size > 1) {
  console.error(
    `Version mismatch across manifests: ${[...versions].join(', ')}.`,
  )
  process.exit(1)
}

console.log(`Versions in sync: ${sources[0].version}.`)
