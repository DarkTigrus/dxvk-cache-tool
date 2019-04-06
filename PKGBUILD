pkgname=dxvk-cache-tool-git
pkgver=0.3.0
pkgrel=1
pkgdesc='Standalone dxvk-cache merger'
url='https://github.com/DarkTigrus/dxvk-cache-tool/'
arch=('x86_64')
license=('MIT' 'Apache')
makedepends=('git' 'rust' 'cargo')
provides=("dxvk-cache-tool=$pkgver")
conflicts=("dxvk-cache-tool")
source=("$pkgname::git+https://github.com/DarkTigrus/dxvk-cache-tool.git")
md5sums=('SKIP')

pkgver() {
  cd $pkgname
  printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short HEAD)"
}

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