const sharp = require('sharp');
const { default: pngToIco } = require('png-to-ico');
const fs = require('fs');
const path = require('path');

const SRC = path.resolve(__dirname, '..', 'icon.jpg');
const OUT = path.resolve(__dirname, '..', 'src-tauri', 'icons');

const sizes = [
  { name: '32x32.png', size: 32 },
  { name: '128x128.png', size: 128 },
  { name: '128x128@2x.png', size: 256 },
  { name: 'icon.png', size: 512 },
  { name: 'Square30x30Logo.png', size: 30 },
  { name: 'Square44x44Logo.png', size: 44 },
  { name: 'Square71x71Logo.png', size: 71 },
  { name: 'Square89x89Logo.png', size: 89 },
  { name: 'Square107x107Logo.png', size: 107 },
  { name: 'Square142x142Logo.png', size: 142 },
  { name: 'Square150x150Logo.png', size: 150 },
  { name: 'Square284x284Logo.png', size: 284 },
  { name: 'Square310x310Logo.png', size: 310 },
  { name: 'StoreLogo.png', size: 50 },
];

async function main() {
  // Generate all PNG sizes
  for (const { name, size } of sizes) {
    await sharp(SRC)
      .resize(size, size, { fit: 'contain', background: { r: 0, g: 0, b: 0, alpha: 0 } })
      .png()
      .toFile(path.join(OUT, name));
    console.log(`✓ ${name} (${size}x${size})`);
  }

  // Generate proper ICO from multiple PNG sizes
  const icoBuf = await pngToIco([
    path.join(OUT, '32x32.png'),
    path.join(OUT, '128x128.png'),
    path.join(OUT, '128x128@2x.png'),
    path.join(OUT, 'icon.png'),
  ]);
  fs.writeFileSync(path.join(OUT, 'icon.ico'), icoBuf);
  console.log('✓ icon.ico (proper ICO format)');

  // ICNS - copy icon.png as base (tauri handles conversion on build)
  fs.copyFileSync(path.join(OUT, 'icon.png'), path.join(OUT, 'icon.icns'));
  console.log('✓ icon.icns');

  console.log('\nDone! All icons generated.');
}

main().catch(console.error);
