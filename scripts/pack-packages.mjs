import { exec, execFile, execSync } from 'node:child_process'
import fs from 'node:fs/promises'
import { platform } from 'node:os'
import path, { dirname } from 'node:path'
import url from 'node:url'

const __dirname = path.dirname(url.fileURLToPath(import.meta.url))
let root = path.resolve(__dirname, '..')
const CFFI_FILE_EXTS = new Set(['.so', '.dylib', '.dll', '.a', '.lib', '.pdb'])
const CFFI_TARGET_ALIASES = new Map([
  ['aarch64-apple-darwin', 'darwin-arm64'],
  ['x86_64-apple-darwin', 'darwin-x64'],
  ['aarch64-pc-windows-msvc', 'win32-arm64-msvc'],
  ['x86_64-pc-windows-msvc', 'win32-x64-msvc'],
  ['aarch64-linux-android', 'android-arm64'],
  ['armv7-linux-androideabi', 'android-arm-eabi'],
  ['aarch64-unknown-linux-gnu', 'linux-arm64-gnu'],
  ['aarch64-unknown-linux-musl', 'linux-arm64-musl'],
  ['armv7-unknown-linux-gnueabihf', 'linux-arm-gnueabihf'],
  ['x86_64-unknown-linux-gnu', 'linux-x64-gnu'],
  ['x86_64-unknown-linux-musl', 'linux-x64-musl'],
  ['x86_64-unknown-freebsd', 'freebsd-x64'],
])

let command = platform() === 'win32' ? 'cd' : 'pwd'
let rawPaths = execSync(`pnpm --silent --filter=!./playgrounds/* -r exec ${command}`).toString()

let paths = rawPaths
  .trim()
  .split(/\r?\n/)
  .map((x) => path.join(x, 'package.json'))

let workspaces = new Map()

// Track all the workspaces
for (let path of paths) {
  let pkg = await fs.readFile(path, 'utf8').then(JSON.parse)
  if (pkg.private) continue
  workspaces.set(pkg.name, { version: pkg.version ?? '', dir: dirname(path) })
}

// Clean dist folder
await fs.rm(path.join(root, 'dist'), { recursive: true, force: true })

await Promise.all(
  [...workspaces.entries()].map(async ([name, { dir }]) => {
    function pack() {
      return new Promise((resolve) => {
        exec(
          `pnpm pack --pack-gzip-level=0 --pack-destination="${path.join(root, 'dist').replace(/\\/g, '\\\\')}"`,
          { cwd: dir },
          (err, stdout, stderr) => {
            if (err) {
              console.error(err, stdout, stderr)
            }

            resolve(lastLine(stdout.trim()))
          },
        )
      })
    }

    let filename = await pack()
    // Remove version suffix
    await fs.rename(
      path.join(root, 'dist', path.basename(filename)),
      path.join(root, 'dist', pkgToFilename(name)),
    )
  }),
)

await packCffiArtifacts()

console.log('Done.')

function pkgToFilename(name) {
  return `${name.replace('@', '').replace('/', '-')}.tgz`
}

function lastLine(str) {
  let index = str.lastIndexOf('\n')
  if (index === -1) return str
  return str.slice(index + 1)
}

async function packCffiArtifacts() {
  const artifactsRoot = path.join(root, 'crates', 'node')
  let entries = []

  try {
    entries = await fs.readdir(artifactsRoot, { withFileTypes: true })
  } catch {
    // ignore
  }

  let cffiDirs = entries
    .filter((entry) => entry.isDirectory() && entry.name.startsWith('cffi-'))
    .map((entry) => ({
      target: entry.name.slice('cffi-'.length),
      dir: path.join(artifactsRoot, entry.name),
    }))

  const stagedLocals = await stageLocalCffiArtifacts()
  cffiDirs.push(...stagedLocals)

  if (cffiDirs.length === 0) {
    return
  }

  await fs.mkdir(path.join(root, 'dist'), { recursive: true })

  for (let { target, dir } of cffiDirs) {
    let archiveName = cffiArchiveName(target)
    let destPath = path.join(root, 'dist', archiveName)
    await createTarballFromDir(dir, destPath)
  }
}

function createTarballFromDir(sourceDir, destPath) {
  return new Promise((resolve, reject) => {
    execFile('tar', ['-czf', destPath, '-C', sourceDir, '.'], (error, stdout, stderr) => {
      if (error) {
        console.error(`Failed to package CFFI artifacts from ${sourceDir}`)
        console.error(stderr || error)
        reject(error)
        return
      }
      resolve()
    })
  })
}

async function stageLocalCffiArtifacts() {
  const headerPath = path.join(root, 'crates', 'cffi', 'include', 'tailwindcss_oxide.h')
  try {
    await fs.access(headerPath)
  } catch {
    return []
  }

  const stageRoot = path.join(root, 'target', '.cffi-pack')
  await fs.mkdir(stageRoot, { recursive: true })

  const staged = []

  // Scan target/<triple>/release directories
  const targetRoot = path.join(root, 'target')
  let targetEntries = []
  try {
    targetEntries = await fs.readdir(targetRoot, { withFileTypes: true })
  } catch {
    targetEntries = []
  }

  const seenTargets = new Set()

  for (const entry of targetEntries) {
    if (!entry.isDirectory()) continue
    if (entry.name.startsWith('.')) continue

    const releaseDir = path.join(targetRoot, entry.name, 'release')
    const artifacts = await collectCffiArtifacts(releaseDir)
    if (artifacts.length === 0) continue

    const stageDir = path.join(stageRoot, `cffi-${entry.name}`)
    await fs.rm(stageDir, { recursive: true, force: true })
    await fs.mkdir(stageDir, { recursive: true })
    await fs.copyFile(headerPath, path.join(stageDir, 'tailwindcss_oxide.h'))
    for (const file of artifacts) {
      await fs.copyFile(path.join(releaseDir, file), path.join(stageDir, file))
    }
    staged.push({ dir: stageDir, target: entry.name })
    seenTargets.add(entry.name)
  }

  // Also consider target/release (host build without triple)
  const hostReleaseDir = path.join(root, 'target', 'release')
  const hostArtifacts = await collectCffiArtifacts(hostReleaseDir)
  if (hostArtifacts.length > 0) {
    const hostTarget = detectHostTarget() ?? 'host'
    if (!seenTargets.has(hostTarget)) {
      const stageDir = path.join(stageRoot, `cffi-${hostTarget}`)
      await fs.rm(stageDir, { recursive: true, force: true })
      await fs.mkdir(stageDir, { recursive: true })
      await fs.copyFile(headerPath, path.join(stageDir, 'tailwindcss_oxide.h'))
      for (const file of hostArtifacts) {
        await fs.copyFile(path.join(hostReleaseDir, file), path.join(stageDir, file))
      }
      staged.push({ dir: stageDir, target: hostTarget })
    }
  }

  return staged
}

function detectHostTarget() {
  try {
    const output = execSync('rustc -vV').toString()
    const hostLine = output.split(/\r?\n/).find((line) => line.startsWith('host:'))
    if (hostLine) {
      return hostLine.split(':')[1].trim()
    }
  } catch {
    // ignore
  }
  return process.env.RUST_TARGET || process.env.CARGO_BUILD_TARGET || null
}

async function collectCffiArtifacts(releaseDir) {
  let entries
  try {
    entries = await fs.readdir(releaseDir)
  } catch {
    return []
  }

  return entries.filter((file) => {
    return file.includes('tailwindcss_oxide_cffi') && CFFI_FILE_EXTS.has(path.extname(file))
  })
}

function cffiArchiveName(target) {
  const label = CFFI_TARGET_ALIASES.get(target) ?? target
  return `tailwindcss-oxide-cffi-${label}.tar.gz`
}
