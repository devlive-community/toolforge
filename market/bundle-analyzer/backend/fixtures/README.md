# Metafile fixtures

Generated in a disposable directory with the official esbuild 0.25.12 API:
`build({ entryPoints: ['src/a.js', 'src/b.js'], bundle: true, splitting: true,
format: 'esm', entryNames: '[name]-[hash]', chunkNames: 'chunk-[hash]',
outdir: 'dist', external: ['remote-pkg'], metafile: true, write: false })`.

The fixture contains shared and dynamic chunks, a CSS bundle, and an external
import. The after build changes the shared label string. The unsplit fixture
uses `splitting: false` and shows an input contributing to multiple outputs.

The accompanying `.analyze.txt` files are unmodified results of
`analyzeMetafile(metafile, { verbose: true, color: false })`. The before report
has 460 emitted bytes, 248 attributed bytes and 212 unattributed bytes. Its A
entry output has 194 bytes, with 134 attributed to `src/a.js`, matching the
134b / 69.1% official analysis. After totals are 476 / 264 / 212 bytes.

No build tools or sources are bundled into the plugin. These files are test
data; analysis does not read any of the paths referenced by them.

Format and semantics: https://esbuild.github.io/api/#metafile
Official analysis: https://esbuild.github.io/api/#analyze
