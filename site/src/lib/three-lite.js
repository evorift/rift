// Hero sahnesinin kullandığı Three.js sembolleri — ve yalnız onlar.
//
// Neden bu dosya var: BlackHoleHero, Three.js'i asenkron yüklemek için dinamik
// import kullanıyor. `await import("three")` doğrudan çağrıldığında paketleyici tüm
// namespace'i materyalize etmek zorunda kalır ve tree-shaking devre dışı kalır
// (ölçüldü: 715 KB). Dinamik sınırı buraya taşıyınca statik yeniden dışa aktarım
// zinciri budanabilir hale gelir.
export {
  WebGLRenderer,
  Scene,
  OrthographicCamera,
  PlaneGeometry,
  ShaderMaterial,
  Mesh,
  BufferGeometry,
  BufferAttribute,
  Points,
  Vector2,
  Color,
  CanvasTexture,
  LinearFilter,
  NormalBlending,
} from "three";
