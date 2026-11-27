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
ab-model-roof-malformed = "{ $name }" çatısı okunamıyor: okunabilir 3 ile 256 arasında kenarı olmalı.
ab-model-roof-footprint = "{ $name }" çatısının taban hattı basit bir döngü değil: kendini kesiyor, kendine değiyor veya bir milimetreden kısa bir kenarı var. Yeniden çizin ya da izlediği duvarları taşıyın.
ab-model-roof-outline = Saçak taşmalarıyla birlikte "{ $name }" çatısının dış hattı kendini kesiyor veya kendine değiyor. Oradaki taşmaları küçültün.
ab-model-roof-no-slope = "{ $name }" çatısının hiçbir kenarı eğim tanımlamıyor. En az bir kenara eğim verin.
ab-model-roof-collinear-slopes = "{ $name }" çatısının iki komşu kenarı ({ $x }, { $y }) yakınında aynı doğru üzerinde ve eğimleri farklı. Aynı eğimi verin veya aradaki noktayı kaldırın.
ab-model-roof-unclosed = "{ $name }" çatısı ({ $x }, { $y }) yakınında kesintisiz bir yüzeyle örtülemiyor: o bölüm yalnızca eğimsiz kenarlarla çevrili veya eğimsiz bir kenar içe dönük bir köşeye ulaşıyor. O kenara eğim verin.
ab-model-roof-degenerate = "{ $name }" çatısının yüzeyi ({ $x }, { $y }) yakınında güvenilir biçimde hesaplanamadı. Oradaki çok kısa veya neredeyse paralel kenarları kaldırın.
ab-model-wall-attach-failed = "{ $name }" duvarı "{ $target }" altını izleyemiyor: bir bölümü onun dışında kalıyor veya alt yüzey duvarın tabanına iniyor. Bu değişene kadar kendi üst kotunu korur.
ab-model-wall-attach-target = "{ $name }" duvarının üstü yalnızca binasındaki bir çatıya veya döşemeye bağlanabilir.
ab-model-wall-shape-invalid = "{ $name }" duvarının biçimi katı oluşturmuyor: yay duvarın iç yüzünün yarıçapı en az 5 cm olmalı, eğimli duvar üstte kalınlığını korumalı ve yay duvar eğimlenemez.
ab-model-wall-profile-invalid = "{ $name }" duvarının profili kullanılamıyor: her halka en az üç nokta içermeli ve kenarları kesişmemeli, dış hat duvarın dikdörtgeni içinde, boşluklar dış hat içinde kalmalı.
ab-model-wall-profile-inserts = Profil, "{ $name }" duvarının { $count } kapı veya pencere boşluğunu duvarın dışında bırakır: { $inserts }.
ab-model-wall-shape-combination = "{ $name }" duvarında { $first } ile { $second } birleştirilemez.
ab-model-wall-band-invalid = "{ $name }" duvarının bir bandı duvara uymuyor: aralığı duvarın içinde kalmalı, genişliği ve derinliği sıfırdan büyük olmalı.
ab-model-wall-stack-invalid = "{ $name }" katmanlı duvar tipi oluşturulamıyor: { $reason ->
    [count] iki ile on altı arasında bölümü olmalı.
    [variable] tam olarak bir bölümün yüksekliği değişken olmalı.
    [kind] her bölüm, kendisi katmanlı olmayan temel bir duvar tipi olmalı.
    [thickness] her bölümün toplam kalınlığı aynı olmalı.
    *[other] bir bölüm kullanılamıyor.
}
ab-model-ceiling-malformed = "{ $name }" tavanının kullanılabilir bir sınırı yok: her halkanın izin verilen aralıkta en az üç noktası olmalı ve kenarları okunabilmeli.
ab-model-ceiling-boundary = "{ $name }" tavanının sınırı kullanılamıyor: bir halka kendini kesiyor ya da değiyor, bir kenar bir milimetreden kısa veya alan aralık dışında. Yeniden çizin.
ab-model-ceiling-edge-unsupported = "{ $name }" tavanının bir kenarı "{ $wall }" duvarını izliyor ama uçlarının ötesine geçiyor: duvar artık tavanı orada sınırlamıyor.
ab-model-opening-footprint = "{ $name }" izdüşümü kullanılamıyor: en az üç noktası olmalı, kendini kesmemeli ve alanı olmalı.
ab-model-opening-outside-host = "{ $name }" boşluğu, onu barındıran "{ $host }" elemanının dışına taşıyor: dışarıdaki kısım hiçbir şey kesmez.
ab-model-floor-slope = "{ $name }" döşemesinin eğimi 85°'den dik. Daha düşük bir eğim, daha uzun bir ok ya da daha küçük bir uç yüksekliği verin.
ab-model-ceiling-slope = "{ $name }" tavanının eğimi 85°'den dik. Daha düşük bir eğim, daha uzun bir ok ya da daha küçük bir uç yüksekliği verin.
ab-model-ceiling-range = "{ $name }" tavanının alt yüzü katın yüksekliğinin dışına çıkıyor: kotun altına ya da üstündeki kotun üzerine geçiyor. Eğim okunu taşıyın ya da eğimi azaltın.
ab-model-ceiling-range-element = "{ $name }" tavanının alt yüzü üstündeki "{ $over }" öğesinin içinden geçiyor. Tavanı ya da eğimi alçaltın.

ab-model-view-range = "{ $name }" plan görünümünün aralığı sırasız: görünüm derinliği alt sınırın üzerinde olamaz, alt sınır kesme yüksekliğinin altında, üst sınır ise üzerinde olmalıdır.
ab-model-floor-edge-profile = "{ $name }" kenar elemanı için en az bir döşeme kenarı ve en az üç noktalı, kendini kesmeyen bir profil gerekir.
ab-model-floor-edge-orphan = "{ $name }" kenar elemanı, döşemesinde artık bulunmayan { $count } kenarı adlandırıyor. Kenarları yeniden seçin ya da elemanı silin.
ab-model-roof-extrusion-profile = "{ $name }" çatısının profili en az iki nokta, sıfır uzunlukta segment içermeyen ve her noktası sınırlar içinde olmalı.
ab-model-roof-extrusion-section = "{ $name }" çatısının kalınlığı profilinin kıvrımları için fazla büyük: kesiti kendini keserdi. Daha küçük bir kalınlık ya da daha yumuşak bir profil verin.
ab-model-roof-extrusion-span = "{ $name }" çatısının ekstrüzyonu 10 mm ile 500 m arasında olmalı.

ab-model-ramp-no-top = "{ $name }" rampasının tabanının üstünde varacağı bir seviye ve bir yükseklik farkı yok. Üste bir kat ekleyin, bir üst seviye seçin ya da yükseklik farkı verin.
ab-model-ramp-malformed = "{ $name }" rampası oluşturulamıyor: genişlik, uzunluk, sahanlık ya da kalınlık geçerli değil.
ab-model-ramp-slope = "{ $name }" rampasının eğimi % { $slope }; tipinin en büyük eğimi olan % { $max } değerini aşıyor. Rampayı uzatın ya da tipi değiştirin.
ab-model-railing-malformed = "{ $name }" korkuluğu oluşturulamıyor: yolu iki farklı nokta gerektirir ve bağlı olduğu elemanın durması gerekir.
ab-model-railing-gap = "{ $name }" korkuluğunda baluster araları { $gap } mm; { $max } mm sınırını aşıyor. Daha küçük baluster aralığı kullanın.
ab-model-location-distant = { $count } öğe iç orijinden en çok { $distance } m uzakta ve { $limit } m'lik duyarlılık uzaklığını aşıyor; çizim ve seçim duyarlılığı düşer. Modeli ölçüm noktasına göre yerleştirin (Modeli Merkezle) ya da duyarlılık uzaklığını artırın.
ab-model-datum-degenerate = "{ $name }" datumunun kullanılabilir bir çizgisi ya da düzlemi yok: akslar ve referans düzlemleri en az bir milimetre uzunlukta olmalı, çalışma düzlemi için datumu ya da aynı doğru üzerinde olmayan üç nokta gerekir.
ab-model-datum-label-duplicate = "{ $name }" etiketini binadaki { $count } aks kullanıyor. Birini yeniden adlandırın.
ab-model-curtain-rule = Bir perde duvar tipinin ızgara kuralı çizgi yerleştiremiyor: değer pozitif bir sayı olmalı, sabit sayı en çok 150 olabilir, açı her yönde en çok 60 derece olmalı ve iki yön neredeyse paralel kalmamalı.
ab-model-curtain-panel = Perde duvar tipinin sistem paneli camlı, dolu ya da boştur. Kapı veya pencereyi gözün paneli olarak o göze verin.
ab-model-curtain-mullion = Bir perde duvar tipinde boyutu kullanılamaz bir doğrama ya da panel var: genişlik ve derinlik pozitif ve sınırlar içinde olmalı.
ab-model-curtain-limit = "{ $name }" perde duvarı, bir duvarın taşıyabileceğinden fazla ızgara düzenlemesi ya da panel seçimi tutuyor.
ab-model-curtain-insert = "{ $name }" perde duvarı bir kapı, pencere ya da boşluk barındırıyor. Bunun yerine duvarın bir gözüne kapı veya pencere paneli verin.
ab-model-curtain-cell-shape = "{ $name }" perde duvarında kaldırılan ızgara parçaları dikdörtgen olmayan bir göz bırakıyor. İki göz arasındaki parçanın tamamını kaldırın.
ab-model-curtain-not-curtain = "{ $name }" bir perde duvar değil.
ab-model-curtain-unknown = "{ $name }" bu perde duvarın bir ızgara çizgisi, gözü ya da tipi değil.
