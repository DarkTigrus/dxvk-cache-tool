pkgname=dxvk-cache-tool
pkgver=1.1.0
pkgrel=1
pkgdesc='Standalone dxvk-cache merger'
url='https://github.com/DarkTigrus/dxvk-cache-tool/'
arch=('x86_64')
license=('MIT' 'Apache')
makedepends=('git' 'rust' 'cargo')
source=("$pkgname::git+https://github.com/DarkTigrus/dxvk-cache-tool.git#tag=v1.1.0")
md5sums=('SKIP')

build() {
  cd $srcdir/$pkgname
  cargo build --release --locked
}

package() {
  cd ${pkgname}
  install -Dm755 target/release/dxvk-cache-tool -t "${pkgdir}/usr/bin"
  install -Dm644 LICENSE-APACHE -t "${pkgdir}/usr/share/licenses/${pkgname}/"
  install -Dm644 LICENSE-MIT -t "${pkgdir}/usr/share/licenses/${pkgname}/"
}

# vim: ts=2 sw=2 et: