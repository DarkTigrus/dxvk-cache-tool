pkgname=dxvk-cache-tool
pkgver=1.1.1
pkgrel=1
pkgdesc='Standalone dxvk-cache merger'
url='https://github.com/DarkTigrus/dxvk-cache-tool/'
arch=('x86_64')
license=('MIT' 'Apache')
makedepends=('git' 'rust' 'cargo')
source=("$pkgname-$pkgver.tar.gz::https://github.com/DarkTigrus/dxvk-cache-tool/archive/v$pkgver.tar.gz")
md5sums=('SKIP')

build() {
  cd $srcdir/$pkgname-$pkgver
  cargo build --release --locked
}

package() {
  cd $srcdir/$pkgname-$pkgver
  install -Dm755 target/release/$pkgname -t "${pkgdir}/usr/bin"
  install -Dm644 LICENSE-APACHE -t "${pkgdir}/usr/share/licenses/${pkgname}/"
  install -Dm644 LICENSE-MIT -t "${pkgdir}/usr/share/licenses/${pkgname}/"
}

# vim: ts=2 sw=2 et: