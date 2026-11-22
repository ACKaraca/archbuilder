# archbuilder-model hata kodlarının iletileri. Tanımlayıcılar locales/en-US/archbuilder-model.ftl ile aynıdır.

ab-model-name-empty = Ad boş olamaz.
ab-model-name-taken = "{ $name }" adı burada zaten kullanılıyor. Adlar bir bina, arsa ya da proje içinde yinelenemez; büyük küçük harf ayrımı yapılmaz.
ab-model-parent-invalid = "{ $name }" uygun türde bir kapsayıcının içinde değil. Arsa projeye, bina arsaya; kat ve seviye binaya, duvar kata, kapı ve pencere ise duvara aittir.
ab-model-project-not-unique = Bir belgede yalnızca bir proje bulunur.
ab-model-level-invalid = "{ $name }" katının, aynı binaya ait ve kat seviyesi olarak işaretlenmiş kendi seviyesi olmalıdır.
ab-model-wall-too-short = "{ $name }" duvarının uzunluğu yok. Eksen çizgisi birbirinden en az bir milimetre uzakta iki farklı noktadan oluşmalıdır.
ab-model-level-below-walls = Seviye oraya taşınamaz: { $count } duvarın (örneğin "{ $name }") üstü tabanının üzerinde kalmaz. Önce o duvarları taşıyın ya da yeniden bağlayın.
ab-model-wall-top-invalid = "{ $name }" duvarının üstü tabanının üzerinde değil. Duvarı daha yüksek bir seviyeye bağlayın ya da bir yükseklik verin.
ab-model-insert-outside = "{ $name }", "{ $wall }" duvarına sığmıyor: açıklık, duvar boyunca köşelerden uzak, { $from } m ile { $to } m arasında olmalıdır.
ab-model-insert-too-tall = "{ $name }", "{ $wall }" duvarında çok yükseğe çıkıyor: açıklığın üstü { $top } m seviyesinin altında kalmalıdır.
ab-model-insert-overlap = "{ $name }", aynı duvardaki "{ $other }" ile çakışıyor. Taşıyın ya da daraltın.
ab-model-insert-operation = "{ $name }" { $operation } olarak çalışamaz: kapı tek ya da çift kanatla açılır ya da kayar, pencere sabit, menteşeli ya da sürmelidir.
ab-model-insert-frame = "{ $name }" kasası net açıklık bırakmıyor. Açıklığı büyütün ya da kasa çubuklarını daraltın.
ab-model-insert-meets-wall = "{ $name }", bulunduğu duvarla orada birleşen ya da kesişen "{ $other }" duvarının içine açılırdı. O duvardan uzağa taşıyın.
ab-model-floor-malformed = "{ $name }" döşemesinin kullanılabilir bir sınırı yok: her halka izin verilen aralıkta en az üç nokta içermeli ve kenarları okunabilmeli.
ab-model-floor-edge-short = "{ $name }" döşemesinin bir milimetreden kısa bir kenarı var. Noktaları birbirinden uzaklaştırın ya da birini silin.
ab-model-floor-self-intersecting = "{ $name }" döşemesinin sınırı kendini kesiyor ya da kendine değiyor. Kenarlar kesişmeyecek biçimde yeniden çizin.
ab-model-floor-area = "{ $name }" döşemesinin kullanılabilir bir alanı yok. Dış sınır en az 1 cm² ve en çok 1 000 km² kapsamalı.
ab-model-floor-hole-outside = "{ $name }" döşemesinin bir boşluğu dış sınırın içinde değil ya da sınıra değiyor. Boşluğu kenarlardan uzak tutun.
ab-model-floor-holes-overlap = "{ $name }" döşemesinin iki boşluğu çakışıyor ya da biri ötekinin içinde. Boşlukları birbirinden ayırın.
ab-model-wall-corner-open = "{ $name }" ile "{ $other }" duvarlarının uçları birbirine yakın ama birleşik değil: köşe açık.
ab-model-floor-edge-unsupported = "{ $name }" döşemesinin bir kenarı "{ $wall }" duvarını izliyor ama uçlarının ötesine taşıyor: duvar döşemeyi orada artık sınırlamıyor.

ab-model-room-unenclosed = "{ $name }" ({ $number }) odası kapalı bir duvar halkasının içinde değil: duvarları kapanana kadar alanı yok.
ab-model-room-redundant = "{ $name }" ({ $number }) odası bölgesini daha düşük numaralı bir odayla paylaşıyor; bölge o odada kalıyor: diğeri kaldırılana ya da bu oda taşınana kadar bu odanın alanı yok.
ab-model-room-number-duplicate = "{ $name }" odasının { $number } numarasını aynı kattaki başka bir oda da kullanıyor.
ab-model-wall-overlap = "{ $name }" ile "{ $other }" üst üste duruyor. Odalar ve döşemeler bunları tek duvar sayar; birini silin.
ab-model-stair-no-top = "{ $name }" merdiveninin tabanının üstünde varacağı bir seviye yok. Üste bir kat ekleyin ya da bir üst seviye seçin.
ab-model-stair-risers = "{ $name }" merdiveninin { $count } rıhtı olurdu; en az { $least }, en çok { $most } gerekir. En yüksek rıhtı ya da seviyeleri değiştirin.
ab-model-stair-malformed = "{ $name }" merdiveninin kaydı okunamıyor.
ab-model-stair-stride = "{ $name }" merdiveninin adım ölçüsü (iki rıht artı basamak) { $stride } m, { $low }–{ $high } m dışında: çıkması rahatsız olur.
ab-model-stair-opening-uncut = "{ $name }" merdiveninin boşluğu "{ $floor }" döşemesine ulaşıyor ama döşemeden kesilemiyor.
ab-model-stair-overlap = "{ $name }" ve "{ $other }" merdivenleri planda üst üste biniyor. Birini taşıyın.

ab-model-type-layer-count = Bir tipin 1 ile { $most } arasında katmanı olur; bunun { $count } katmanı var. Katman ekleyin ya da bazılarını silin.
ab-model-type-layer-thickness = { $layer }. katmanın kalınlığı 0'dan büyük ve en çok { $most } m olmalıdır. Kalınlığı olmayan tek katman membrandır.
ab-model-type-membrane-thickness = { $layer }. katman bir membrandır ve kalınlığı yoktur. Kalınlığını 0 yapın ya da başka bir işlev seçin.
ab-model-type-core = Çekirdekte en az bir katman olmalıdır: art arda gelen bir ya da daha çok katmanın Çekirdek sütununu işaretleyin.
ab-model-type-total = Katmanların toplamı { $total } mm; toplam { $least } mm ile { $most } mm arasında olmalıdır.
ab-model-type-mismatch = "{ $name }", { $field } değerinde tipiyle uyuşmuyor. Bu değer tipten gelir: tipi düzenleyin ya da başka bir tip seçin.
ab-model-type-operation = "{ $name }" tipi { $operation } olarak çalışamaz: kapı tek ya da çift kanatla açılır ya da kayar, pencere sabit, menteşeli ya da sürmelidir.
ab-model-pinned = "{ $name }" sabitlenmiş. Değiştirmek için sabitlemesini kaldırın (UP).
