# archbuilder-commands iletileri: hata kodları ve komut etiketleri. Tanımlayıcılar locales/en-US/archbuilder-commands.ftl ile aynıdır.

ab-cmd-document-not-empty = Zaten projesi olan bir belgede yeni proje oluşturulamaz.
ab-cmd-kind-mismatch = Bu komut, bu türdeki bir nesne üzerinde çalışmaz.
ab-cmd-no-building = Belgede kat eklenecek bir bina yok.
ab-cmd-nothing-selected = Önce nesneleri seçin.
ab-cmd-field-unknown = "{ $field }" alanı değiştirilemez.
ab-cmd-value-invalid = Değer bu alana uygun değil.
ab-cmd-last-storey = Binanın son katı silinemez. Bir binada en az bir kat bulunur.

ab-cmd-project-new-label = Yeni proje
ab-cmd-project-new-description = Bir arsa, bir bina ve seviyeleriyle birlikte üç kat içeren proje oluşturur.
ab-cmd-storey-add-label = Kat ekle
ab-cmd-storey-add-description = Binaya bir kat ve seviyesi ekler; kot verilmezse en yüksek seviyenin üzerine koyar.
ab-cmd-storey-rename-label = Katı yeniden adlandır
ab-cmd-storey-rename-description = Bir katı ve seviyesini yeniden adlandırır.
ab-cmd-storey-delete-label = Katı sil
ab-cmd-storey-delete-description = Bir katı ve seviyesini siler. Binanın son katı silinemez.
ab-cmd-building-rename-label = Binayı yeniden adlandır
ab-cmd-building-rename-description = Bir binayı yeniden adlandırır.
ab-cmd-level-set-elevation-label = Kotu değiştir
ab-cmd-level-set-elevation-description = Bir seviyenin ya da bir katın seviyesinin kotunu değiştirir.
ab-cmd-object-rename-label = Yeniden adlandır
ab-cmd-object-rename-description = Bir projeyi, arsayı, binayı, katı ya da seviyeyi yeniden adlandırır.
ab-cmd-object-select-label = Seç
ab-cmd-object-select-description = Nesneleri seçer; bir seviye çizgisi, katını seçer.
ab-cmd-property-set-label = Özelliği değiştir
ab-cmd-property-set-description = Mekânsal yapıdaki nesnelerin bir alanını değiştirir; Özellikler panelinin komutudur.

ab-cmd-storey-has-walls = Kat silinemez, çünkü üzerinde duvar ya da döşeme var (adet: { $count }). Önce onları silin.
ab-cmd-wall-end-same = Bitiş noktası başlangıç noktasıyla aynı. Duvarın bir uzunluğu olmalı.
ab-cmd-wall-create-label = Duvar
ab-cmd-wall-create-description = Bir kata, iki noktalı bir eksen çizgisinden; kalınlık, kat seviyesinden başlayan yükseklik ve konum çizgisiyle duvar yerleştirir.
ab-cmd-wall-delete-label = Duvarı Sil
ab-cmd-wall-delete-description = Duvarları siler; duvar belirtilmezse seçili olanları.
ab-cmd-wall-flip-label = Duvarı Çevir
ab-cmd-wall-flip-description = Duvarları çevirir: eksen çizgisi ters çevrilir, dış taraf yer değiştirir; konum çizgisi aynı kalır.
ab-cmd-wall-move-end-label = Duvar Ucunu Taşı
ab-cmd-wall-move-end-description = Bir duvarın bir ucunu bir noktaya taşır; öbür uç yerinde kalır, kapı ve pencereler duvar boyunca yerlerini korur, korumazsa taşıma onları adlandırarak reddedilir.
ab-cmd-object-delete-label = Sil
ab-cmd-object-delete-description = Seçili duvarları siler; duvar seçili değilse seçili katı siler.

ab-prompt-wall-start = { $storey } üzerinde duvar: başlangıç noktasını belirtin (kalınlık { $thickness }, yükseklik { $height }, çizgi { $location ->
        [exterior] dış yüz
        [interior] iç yüz
        [core_center] çekirdek ekseni
        [core_exterior] çekirdek dış yüzü
        [core_interior] çekirdek iç yüzü
       *[other] eksen
    })
ab-prompt-wall-end = { $storey } üzerinde duvar: sonraki noktayı belirtin ya da bitirmek için Enter'a basın (kalınlık { $thickness }, yükseklik { $height }, çizgi { $location ->
        [exterior] dış yüz
        [interior] iç yüz
        [core_center] çekirdek ekseni
        [core_exterior] çekirdek dış yüzü
        [core_interior] çekirdek iç yüzü
       *[other] eksen
    })
ab-prompt-wall-thickness = Duvar kalınlığı (şimdi { $thickness })
ab-prompt-wall-height = Duvar yüksekliği (şimdi { $height })
ab-prompt-wall-location = Konum çizgisi
ab-keyword-wall-thickness = Kalınlık
ab-keyword-wall-height = Yükseklik
ab-keyword-wall-location = Konum çizgisi
ab-keyword-wall-flip = Çevir
ab-keyword-wall-close = Kapat
ab-keyword-wall-center = Eksen
ab-keyword-wall-exterior = Dış yüz
ab-keyword-wall-interior = İç yüz
ab-cmd-wall-join-style-label = Duvar Birleşim Biçimi
ab-cmd-wall-join-style-description = Duvarların bir ya da iki ucunun karşılaştığı duvarlarla nasıl birleşeceğini belirler: otomatik, alın, kare ya da birleşmesiz.
ab-cmd-wall-disallow-join-label = Duvar Birleşimini Engelle
ab-cmd-wall-disallow-join-description = Duvar uçlarını kare bırakır ve her birleşimin dışında tutar; duvarlar orada çakışabilir.
ab-cmd-wall-allow-join-label = Duvar Birleşimine İzin Ver
ab-cmd-wall-allow-join-description = Duvar uçlarını otomatik birleşimlere geri alır.
ab-cmd-no-wall-selected = Önce bir duvarı gösterin ya da seçin.
ab-cmd-no-insert-selected = Önce bir kapı ya da pencere seçin.
ab-cmd-door-place-label = Kapı
ab-cmd-door-place-description = Bir duvara, boyunca bir mesafede; genişlik, yükseklik, kanat yönü ve açılış yönüyle kapı yerleştirir.
ab-cmd-window-place-label = Pencere
ab-cmd-window-place-description = Bir duvara, boyunca bir mesafede; genişlik, yükseklik, denizlik yüksekliği, kanat yönü ve açılış yönüyle pencere yerleştirir.
ab-cmd-insert-flip-hand-label = Kanat Yönünü Çevir
ab-cmd-insert-flip-hand-description = Kapı ve pencerelerin kanat yönünü çevirir: menteşe karşı tarafa geçer.
ab-cmd-insert-flip-facing-label = Açılış Yönünü Çevir
ab-cmd-insert-flip-facing-description = Kapı ve pencerelerin açılış yönünü çevirir: kanat duvarın öbür yanına açılır.
ab-prompt-door-place = Kapı ({ $wall }): duvara tıklayın ya da mesafe yazın ({ $width } x { $height })
ab-prompt-window-place = Pencere ({ $wall }): duvara tıklayın ya da mesafe yazın ({ $width } x { $height })
ab-prompt-insert-width = Genişlik (şimdi { $width })
ab-prompt-insert-height = Yükseklik (şimdi { $height })
ab-prompt-insert-sill = Denizlik yüksekliği (şimdi { $sill })
ab-keyword-insert-width = Genişlik
ab-keyword-insert-height = Yükseklik
ab-keyword-insert-sill = Denizlik yüksekliği
ab-keyword-insert-hand = Kanat yönü
ab-keyword-insert-facing = Açılış yönü
ab-cmd-floor-arcs-unsupported = { $name } döşemesinin yay kenarları var ve bu komut onları koruyamaz. Kenarları düz yapın ya da sınırını düzenleyin.
ab-cmd-floor-no-loop = Bu duvarlar kapalı bir sınır oluşturmuyor. Alanı çevreleyen, uç uca gelen duvarları seçin ya da kapalı bir duvar halkasının içine tıklayın.
ab-cmd-floor-walls-parallel = Sınırda birbirini izleyen iki duvar paralel, kesişmiyorlar. Köşelerde buluşan duvarları seçin.
ab-cmd-no-floor-selected = Adı verilmiş ya da seçili bir döşeme yok. Önce bir döşeme seçin.
ab-cmd-floor-too-big = Bir döşeme en çok { $loops } halka ve toplam { $points } nokta alabilir.
ab-cmd-floor-create-label = Döşeme
ab-cmd-floor-create-description = Bir katta; seçilen duvarlardan (kenarlar duvar yüzlerini izler) ya da çizilen bir sınırdan; kalınlığı ve üst yüzünün kat seviyesine göre kot farkıyla döşeme oluşturur.
ab-cmd-floor-edit-boundary-label = Sınırı düzenle
ab-cmd-floor-edit-boundary-description = Döşemenin dış sınırını ve boşluklarını, yaylar ve kenarların izlediği duvarlarla birlikte tek adımda değiştirir.
ab-cmd-floor-add-hole-label = Döşeme boşluğu
ab-cmd-floor-add-hole-description = Çizilen kapalı bir sınırdan döşemede boşluk açar.
ab-prompt-floor-pick = { $storey } üzerinde döşeme: bir duvara ya da kapalı bir duvar halkasının içine tıklayın ({ $face ->
        [exterior] dış yüz
       *[interior] iç yüz
    }, kalınlık { $thickness }, kot farkı { $offset }). { $count } duvar seçildi; bitirmek için Enter'a basın.
ab-prompt-floor-start = { $storey } üzerinde döşeme: sınırın ilk noktasını belirtin (kalınlık { $thickness }, kot farkı { $offset }).
ab-prompt-floor-next = { $storey } üzerinde döşeme: sonraki noktayı belirtin ya da bitirmek için Enter'a basın veya Kapat'ı seçin ({ $count } nokta, kalınlık { $thickness }, kot farkı { $offset }).
ab-prompt-floor-hole-start = { $floor } içinde boşluk: boşluğun ilk noktasını belirtin.
ab-prompt-floor-hole-next = { $floor } içinde boşluk: sonraki noktayı belirtin ya da bitirmek için Enter'a basın veya Kapat'ı seçin ({ $count } nokta).
ab-prompt-floor-thickness = Döşeme kalınlığı (şimdi { $thickness })
ab-prompt-floor-offset = Kat seviyesine göre üst kot farkı (şimdi { $offset })
ab-keyword-floor-thickness = Kalınlık
ab-keyword-floor-offset = Kot farkı
ab-keyword-floor-interior = İç yüz
ab-keyword-floor-exterior = Dış yüz
ab-keyword-floor-sketch = Çiz
ab-keyword-floor-walls = Duvar seç
ab-keyword-floor-done = Bitir
ab-keyword-floor-close = Kapat
ab-keyword-floor-undo = Noktayı geri al

ab-cmd-not-movable = Katlar ve seviyeler taşınamaz, kopyalanamaz, döndürülemez ya da aynalanamaz.
ab-cmd-insert-alone = Tek başına bir kapı ya da pencere yalnızca duvarı boyunca kayabilir. Duvarını onunla birlikte döndürün ya da aynalayın.
ab-cmd-mirror-axis = Ayna çizgisinin iki noktası farklı olmalıdır.
ab-cmd-modify-move-label = Taşı
ab-cmd-modify-move-description = Seçilen duvarları, döşemeleri, kapıları ve pencereleri taşır; kapı ve pencereler duvarını izler, tek başına biri duvarı boyunca kayar.
ab-cmd-modify-copy-label = Kopyala
ab-cmd-modify-copy-description = Seçilen duvarları (kapı ve pencereleriyle) ve döşemeleri bir yer değiştirmeyle kopyalar; kopyalar yeni adlar alır.
ab-cmd-modify-rotate-label = Döndür
ab-cmd-modify-rotate-description = Seçilen nesneleri bir taban noktası çevresinde bir açıyla ya da bir başvuru açısından yeni açıya döndürür.
ab-cmd-modify-mirror-label = Aynala
ab-cmd-modify-mirror-description = Seçilen nesneleri bir çizgiye göre aynalar, özgünleri tutar ya da siler; aynalanan kapı ve pencerelerin yönü değişir.
ab-prompt-modify-base = Taban noktasını belirtin veya [Yer değiştirme]:
ab-prompt-modify-second-move = İkinci noktayı belirtin veya bir uzunluk yazın:
ab-prompt-modify-second-copy = İkinci noktayı belirtin (bitirmek için Enter):
ab-prompt-modify-displacement = Yer değiştirmeyi belirtin (örneğin 12,0):
ab-prompt-rotate-base = Taban noktasını belirtin:
ab-prompt-rotate-angle = Dönme açısını belirtin veya [Başvuru]:
ab-prompt-rotate-reference = Başvuru açısını belirtin:
ab-prompt-rotate-new = Yeni açıyı belirtin:
ab-prompt-mirror-first = Ayna çizgisinin ilk noktasını belirtin:
ab-prompt-mirror-second = Ayna çizgisinin ikinci noktasını belirtin:
ab-prompt-mirror-erase = Özgün nesneler silinsin mi? [Evet/Hayır] <Hayır>:
ab-keyword-modify-displacement = Yer değiştirme
ab-keyword-rotate-reference = Başvuru
ab-keyword-yes = Evet
ab-keyword-no = Hayır
ab-prompt-modify-select = Nesneleri seçin (tıklayın, soldan sağa pencere, sağdan sola kesen pencere), sonra Enter'a basın:
ab-cmd-select-before-points = Nokta vermeden önce nesneleri seçin (tıklayın ya da pencere sürükleyin) ve Enter'a basın.

ab-cmd-room-no-region = Bu nokta kapalı bir duvar halkasının içinde değil. Bir alanı çevreleyen duvarların içine tıklayın.
ab-cmd-room-region-taken = Bu bölgede zaten bir oda var.
ab-cmd-room-none-missing = Bu katta kapalı her bölgede zaten bir oda var.
ab-cmd-room-region-unresolved = Duvarlarla çevrili { $count } bölgenin sınırı çözülemedi, bu yüzden odaları yok. Çevrelerindeki duvarları kontrol edin.
ab-cmd-room-region-sliver = Bu bölge { $area } m², en küçük oda alanı olan { $least } m² değerinden küçük: duvarlar arasında kalan dar bir şerit, oda almaz.
ab-cmd-room-only-slivers = Odası olmayan her bölge en küçük oda alanı olan { $least } m² değerinden küçük ({ $count } dar şerit): oda yerleştirilmedi.
ab-cmd-room-place-label = Oda
ab-cmd-room-place-description = Bir katta, duvarlarla kapanan bir bölgenin içindeki noktaya oda yerleştirir. Sınır, alan, çevre ve hacim duvarları izler; oda katın sıradaki numarasını alır.
ab-cmd-room-place-all-label = Tüm odaları yerleştir
ab-cmd-room-place-all-description = Bir katta, odası olmayan kapalı her bölgeye oda yerleştirir.
ab-prompt-room-place = { $storey } üzerinde oda: duvarlarla kapanan bir bölgenin içine tıklayın, bitirmek için Esc.
ab-cmd-clipboard-paste-label = Yapıştır
ab-cmd-clipboard-paste-description = Panodaki nesnelerin (kapı ve pencereli duvarlar, döşemeler ve odalar) kopyalarını bir ekleme noktasına ya da yerine yapıştırır.
ab-cmd-clipboard-paste-storey-label = Kata Yapıştır
ab-cmd-clipboard-paste-storey-description = Panodaki nesnelerin kopyalarını aynı plan konumunda başka bir kata yapıştırır; kotlar kata göre değişir.
ab-prompt-paste-point = Ekleme noktasını belirtin (yerine yapıştırmak için Enter):
ab-cmd-clipboard-empty = Henüz hiçbir şey kopyalanmadı. Önce duvar, döşeme, kapı, pencere ya da oda seçip kopyalayın.
ab-cmd-storey-not-selected = Önce hedef katı seçin (örneğin yapı ağacından), sonra kata yapıştırın.
ab-cmd-shortcuts-invalid = Dosya bir kısayol tablosu değil ya da iki harf olmayan bir tuşa ya da var olmayan bir komuta bağlanmış.
ab-cmd-object-not-found = { $id } tanıtıcılı bir nesne yok.
ab-cmd-stair-create-label = Merdiven
ab-cmd-stair-create-description = Bir katın seviyesinden üstteki seviyeye merdiven oluşturur: düz ya da sahanlıklı L veya U dönüşlü; genişlik, basamak derinliği ve en yüksek rıht verilir; rıht sayısı ve yüksekliği seviyelerden çıkar.
ab-prompt-stair-start = { $storey } üzerinde merdiven ({ $shape ->
    [l_turn] { $turn ->
        [right] sağa
       *[left] sola
    } L dönüş
    [u_turn] { $turn ->
        [right] sağa
       *[left] sola
    } U dönüş
   *[straight] düz
}, { $risers } rıht × { $riser }, genişlik { $width }, basamak { $tread }): başlangıcı, yani ilk rıhtın ortasını belirtin ya da bir seçenek seçin.
ab-prompt-stair-direction = Merdiven ({ $risers } rıht × { $riser }, genişlik { $width }, basamak { $tread }): ilk kolun yönünü belirtin ya da bir seçenek seçin.
ab-prompt-stair-turn = Çıkarken hangi yana dönsün?
ab-prompt-stair-width = Merdiven genişliği (şimdi { $width })
ab-prompt-stair-tread = Basamak derinliği (şimdi { $tread })
ab-keyword-stair-straight = Düz
ab-keyword-stair-l-turn = L dönüş
ab-keyword-stair-u-turn = U dönüş
ab-keyword-stair-left = Sol
ab-keyword-stair-right = Sağ
ab-keyword-stair-width = Genişlik
ab-keyword-stair-tread = Basamak
ab-prompt-stair-turn-again = "{ $typed }" bir yan değil. Sol (s) ya da Sağ (a) yanıtını verin veya gösterilen yan için Enter'a basın.

ab-cmd-field-from-type = { $field } tipten gelir. Tipi düzenleyin ya da seçili nesneler için başka bir tip seçin.
ab-cmd-type-unknown = "{ $name }" adında bir tip yok. Bu türün tiplerinden birini seçin.
ab-cmd-type-in-use = "{ $name }" tipini { $count ->
        [one] bir nesne
       *[other] { $count } nesne
    } kullanıyor. Yerine geçecek bir tip seçin, sonra silin.
ab-cmd-type-last = "{ $name }" türünün son tipidir ve silinemez. Önce çoğaltın ya da düzenleyin.
ab-cmd-type-kind = Nesneler o tipin türünden değil. Duvar duvar tipi, döşeme döşeme tipi, kapı kapı tipi, pencere pencere tipi alır.
ab-cmd-type-set-label = Tipi Düzenle
ab-cmd-type-set-description = Bir tipin adını, katmanlarını, çekirdeğini, özelliklerini ya da boyutunu değiştirir; tipin bütün nesneleri tek adımda onunla birlikte değişir.
ab-cmd-type-duplicate-label = Tipi Çoğalt
ab-cmd-type-duplicate-description = Bir tipin yeni adla kopyasını oluşturur.
ab-cmd-type-delete-label = Tipi Sil
ab-cmd-type-delete-description = Bir tipi siler; onu kullanan nesneler yerine geçen tipi alır.
ab-cmd-type-assign-label = Tipi Değiştir
ab-cmd-type-assign-description = Seçili duvar, döşeme, kapı ya da pencerelere kendi türlerinden başka bir tip verir.
ab-cmd-type-match-label = Tipi Eşle
ab-cmd-type-match-description = Seçili nesnelere aynı türden bir kaynak nesnenin tipini verir.
ab-cmd-type-upgrade-label = Tip Ekle
ab-cmd-type-upgrade-description = Tipi olmayan her duvar, döşeme, kapı ve pencereye boyutuna uyan bir tip verir; uyan yoksa oluşturur.

ab-type-wall-generic = Genel 200
ab-type-wall-cavity = Dış yığma boşluklu duvar
ab-type-wall-partition = İç bölme 100
ab-type-floor-generic = Genel 200
ab-type-floor-slab = Şap ve kaplamalı betonarme döşeme
ab-type-door-single = Tek kanat 900 × 2100
ab-type-door-double = Çift kanat 1500 × 2100
ab-type-window-large = Pencere 1200 × 1500
ab-type-window-small = Pencere 600 × 600
ab-type-wall-generated = Genel { $mm }
ab-type-floor-generated = Genel { $mm }
ab-type-roof-generic = Genel çatı 300
ab-type-roof-rafters = Yalıtımlı ahşap mertek ve kiremit
ab-type-roof-generated = Çatı { $mm }
ab-material-roof-tiles = Kiremit
ab-material-battens = Ahşap çıta
ab-material-roof-underlay = Çatı altı örtüsü
ab-material-rafters-insulation = Yalıtımlı ahşap mertek
ab-type-door-generated = Kapı { $width } × { $height }
ab-type-window-generated = Pencere { $width } × { $height }
ab-type-copy-name = { $name } ({ $n })
ab-material-generic = Genel
ab-material-brick = Tuğla
ab-material-air = Hava boşluğu
ab-material-insulation = Yalıtım
ab-material-vapor-barrier = Buhar kesici
ab-material-concrete-block = Beton blok
ab-material-plaster = Sıva
ab-material-gypsum-board = Alçı levha
ab-material-steel-stud = Çelik dikme
ab-material-floor-finish = Döşeme kaplaması
ab-material-screed = Şap
ab-material-separation-layer = Ayırma katmanı
ab-material-concrete = Beton
ab-keyword-wall-core-center = Çekirdek ekseni
ab-keyword-wall-core-exterior = Çekirdek dış yüzü
ab-keyword-wall-core-interior = Çekirdek iç yüzü
ab-type-door-sliding = Sürme 1800 × 2100
ab-type-window-sliding = Sürme pencere 1800 × 1200

## Hizala, ötele, kırp, böl, birleştir, dizi, sabitle

ab-cmd-modify-align-label = Hizala
ab-cmd-modify-align-description = Bir elemanı, seçilen çizgisi bir referans çizgisinin üzerine gelecek şekilde döndürür ve taşır: duvar yüzü veya eksen çizgisi ya da döşeme kenarı. Duvarlar birleşimlerini korur, kapı ve pencereleri onları izler.
ab-cmd-modify-offset-label = Ötele
ab-cmd-modify-offset-description = Seçilen duvarları ve döşeme sınırlarını seçilen noktanın tarafına doğru bir mesafe kadar taşır veya kopyalar. Uç uca birleşen duvarlar zincir olarak taşınır.
ab-cmd-modify-trim-label = Köşeye kırp
ab-cmd-modify-trim-description = İki duvarı temiz bir köşede buluşacak şekilde kırpar veya uzatır; her birinin seçilen kısmı kalır.
ab-cmd-modify-trim-to-label = Sınıra kırp
ab-cmd-modify-trim-to-description = Duvarları birer birer seçilen sınır çizgisine kırpar veya uzatır; seçilen kısım kalır.
ab-cmd-modify-trim-many-label = Birden çoğunu sınıra kırp
ab-cmd-modify-trim-many-description = Birden çok duvarı aynı anda seçilen sınır çizgisine kırpar veya uzatır; Enter seçimi bitirir.
ab-cmd-wall-split-label = Duvarı böl
ab-cmd-wall-split-description = Bir duvarı bir noktada veya en yakın ucundan bir mesafede, isteğe bağlı bir boşlukla ikiye böler. Kapı ve pencereler onları taşıyan parçaya geçer.
ab-cmd-wall-merge-label = Duvarları birleştir
ab-cmd-wall-merge-description = Uç uca değen, aynı doğrultudaki, aynı tip ve yükseklikteki iki duvarı tek duvarda birleştirir; kapı ve pencereleri onlarla gider.
ab-cmd-modify-array-label = Doğrusal dizi
ab-cmd-modify-array-description = Seçimin bağımsız kopyalarını bir aralıkla veya son kopyaya olan mesafeyle sıra halinde yapar.
ab-cmd-modify-array-radial-label = Dairesel dizi
ab-cmd-modify-array-radial-description = Seçimin bağımsız kopyalarını bir merkez çevresinde, bir açı boyunca yapar.
ab-cmd-element-pin-label = Sabitle
ab-cmd-element-pin-description = Seçilen elemanları sabitler: onları taşıyacak, biçimini değiştirecek veya silecek komutlar reddedilir.
ab-cmd-element-unpin-label = Sabitlemeyi kaldır
ab-cmd-element-unpin-description = Seçilen elemanların sabitlemesini kaldırır.

ab-prompt-align-reference = Referans çizgisini seçin (duvar yüzü veya eksen çizgisi ya da döşeme kenarı):
ab-prompt-align-target = Ona hizalanacak çizgiyi seçin:
ab-prompt-offset-distance = Öteleme mesafesini yazın veya ötelenecek noktayı seçin ya da [Kopyala/Taşı]:
ab-prompt-offset-side = Ötelenecek tarafı seçin:
ab-keyword-offset-copy = Kopyala
ab-keyword-offset-move = Taşı
ab-prompt-trim-first = Birinci duvarı kalacak kısmından seçin:
ab-prompt-trim-other = İkinci duvarı kalacak kısmından seçin:
ab-prompt-trim-boundary = Sınır çizgisini seçin:
ab-prompt-trim-target = Kırpılacak veya uzatılacak duvarı kalacak kısmından seçin:
ab-prompt-trim-targets = Kırpılacak veya uzatılacak duvarları kalacak kısımlarından seçin; bitince Enter:
ab-keyword-done = Bitti
ab-prompt-split-point = Bölmek için duvar üzerinde bir nokta seçin veya ucundan bir mesafe yazın ya da [Boşluk]:
ab-prompt-split-end = Ölçülecek uca yakın yerden duvarı seçin:
ab-prompt-split-gap = Parçalar arasındaki boşluğun genişliğini yazın:
ab-keyword-split-gap = Boşluk
ab-prompt-merge-first = Kalacak duvarı seçin:
ab-prompt-merge-other = Ona katılacak duvarı seçin:
ab-prompt-array-count = Özgün dahil öğe sayısını yazın:
ab-prompt-array-base = Taban noktasını seçin:
ab-prompt-array-second = İkinci öğenin yerini seçin veya [Son]:
ab-prompt-array-last = Son öğenin yerini seçin veya [Aralık]:
ab-keyword-array-last = Son
ab-keyword-array-spacing = Aralık
ab-prompt-array-center = Merkezi seçin:
ab-prompt-array-angle = Öğelerin kapladığı açıyı yazın veya bir yön seçin (tam tur için Enter):

ab-cmd-no-wall-at = O noktada duvar yok. Bir duvarın üzerinden seçin.
ab-cmd-no-line-at = O noktada duvar yüzü, duvar eksen çizgisi veya döşeme kenarı yok. Birine daha yakın seçin.
ab-cmd-parallel = İki çizgi paralel, hiç kesişmiyor.
ab-cmd-same-wall = Aynı duvar iki kez seçildi. Başka bir duvar seçin.
ab-cmd-split-end = "{ $name }" duvarını orada bölmek çok kısa bir parça bırakır. Uçlarından daha uzakta seçin.
ab-cmd-split-insert = "{ $name }", "{ $wall }" duvarının bölme noktasının üzerinde. Yanından bölün ya da önce onu taşıyın.
ab-cmd-merge-refused = "{ $name }" ile "{ $other }" birleşemez: aynı doğrultuda olmalı, uç uca değmeli ve tipleri, yükseklikleri, tabanları, üstleri ve konum çizgileri aynı olmalı.
ab-cmd-array-insert = "{ $name }" yalnızca kendi duvarı boyunca dizilebilir.
ab-cmd-nothing-to-offset = Ötelenecek duvarları veya döşemeleri seçin ve çizgilerinden uzakta bir taraf seçin.
ab-cmd-roof-create-label = Çatı
ab-cmd-roof-create-description = Bir katta, seçilen duvarlardan (kenarlar duvar yüzlerini izler) veya çizilen bir taban hattından çatı oluşturur. Her kenar ya kendi açısıyla eğimlidir ya da bir kalkan duvarıdır ve bir saçak taşması vardır.
ab-cmd-roof-set-edge-label = Çatı kenarları
ab-cmd-roof-set-edge-description = Çatı kenarlarının eğimli olup olmadığını, eğimlerini ve saçak taşmalarını ayarlar.
ab-cmd-roof-move-vertex-label = Çatı noktasını taşı
ab-cmd-roof-move-vertex-description = Çatı taban hattının bir noktasını taşır; o noktadaki iki kenar duvarları izlemeyi bırakır.
ab-cmd-roof-move-edge-label = Çatı kenarını taşı
ab-cmd-roof-move-edge-description = Çatı taban hattının bir kenarını dışa veya içe taşır; duvarı izleyen kenar yeni bir ötelemeyle izlemeye devam eder.
ab-cmd-wall-attach-top-label = Üstü bağla
ab-cmd-wall-attach-top-description = Seçili duvarların üstünü üzerlerindeki çatıya veya döşemeye bağlar; üst, kalkan uçları ve kırmalar dahil, onun alt yüzünü izler.
ab-cmd-wall-detach-top-label = Üst bağını çöz
ab-cmd-wall-detach-top-description = Seçili duvarların üstünü çatıdan veya döşemeden ayırır; her duvar önceki üst kotunu geri alır.
ab-cmd-no-roof-selected = Adı verilen veya seçili bir çatı yok. Önce bir çatı seçin.
ab-cmd-roof-edge-index = Çatının bu numarada bir kenarı veya noktası yok.
ab-cmd-no-attach-target = "{ $name }" duvarının üzerinde çatı veya döşeme yok. Üzerine bir çatı yerleştirin ya da bağlanacak çatıyı veya döşemeyi belirtin.
ab-cmd-attach-target-kind = Duvar üstü yalnızca bir çatıya veya döşemeye bağlanabilir.
ab-prompt-roof-pick = { $storey } katında çatı: alanın çevresindeki duvarlara sırayla tıklayın veya kapalı bir duvar halkasının içine tıklayın, sonra Bitti'yi seçin (eğim { $slope }, saçak { $overhang }).
ab-prompt-roof-start = { $storey } katında çatı: taban hattının ilk noktasını belirtin (eğim { $slope }, saçak { $overhang }).
ab-prompt-roof-next = { $storey } katında çatı: sonraki noktayı belirtin ya da bitirmek için Enter'a basın veya Kapat'ı seçin ({ $count } nokta, eğim { $slope }, saçak { $overhang }).
ab-prompt-roof-slope = Kenarların eğimi (şu an { $slope }): açı (30°), yüzde (%58) veya yükselti:yatay (1:2); yalın sayı projenin eğim birimindedir
ab-prompt-roof-overhang = Saçak taşması (şu an { $overhang })
ab-prompt-roof-base = Saçak kotunun kata göre yüksekliği
ab-keyword-roof-slope = Eğim
ab-keyword-roof-overhang = Saçak
ab-keyword-roof-base = Saçak kotu
ab-cmd-slope-invalid = Bu eğim kullanılamaz. { $low } ile { $high } arasında bir eğim yazın: açı (30°), yüzde (%58) veya yükselti:yatay (1:2).
ab-cmd-ceiling-create-label = Tavan
ab-cmd-ceiling-create-description = Bir katta, bir odanın içine tıklayarak (sınırı odanın duvar yüzlerindeki bölgesidir), seçilen duvarlardan ya da yaylı çizilmiş bir sınırdan; kat kotunun üstünde bir yükseklikte ve isteğe bağlı eğim okuyla tavan oluşturur.
ab-cmd-ceiling-slope-label = Tavan eğimi
ab-cmd-ceiling-slope-description = Tavana bir eğim oku verir: bir başlangıç çizgisi, bir yön ve bir eğim ya da uç yüksekliği. Düz tavanda yoktur.
ab-cmd-shaft-create-label = Şaft
ab-cmd-shaft-create-description = Çizilmiş bir izdüşümü, her biri bir kot ve ötelemeyle verilen taban ile tepe arasındaki her döşemeden, çatıdan ve tavandan geçirerek keser.
ab-cmd-vertical-opening-create-label = Düşey boşluk
ab-cmd-vertical-opening-create-description = Kendisini barındıran bir döşemede, çatıda ya da tavanda çizilmiş bir boşluk keser.
ab-cmd-opening-place-label = Duvar boşluğu
ab-cmd-opening-place-description = Kapı ya da pencere olmadan, bir duvara boyunca bir mesafede; genişlik, yükseklik, denizlik ve kemer yüksekliğiyle dikdörtgen ya da kemerli bir boşluk keser.
ab-cmd-no-ceiling-selected = Hiçbir tavan adlandırılmadı ya da seçilmedi. Önce bir tavan seçin.
ab-cmd-no-host-selected = Boşluğu barındıracak bir döşeme, çatı ya da tavan adlandırılmadı ya da seçilmedi. Önce birini seçin.
ab-prompt-ceiling-pick = { $storey } üzerinde tavan: bir odanın ya da kapalı bir duvar halkasının içine ya da duvarlara tıklayın ({ $face ->
        [exterior] dış yüz
       *[interior] iç yüz
    }, yükseklik { $offset }, kalınlık { $thickness }). { $count } duvar seçildi; bitirmek için Enter'a basın.
ab-prompt-ceiling-start = { $storey } üzerinde tavan: sınırın ilk noktasını belirtin (yükseklik { $offset }, kalınlık { $thickness }).
ab-prompt-ceiling-next = { $storey } üzerinde tavan: sonraki noktayı belirtin ya da bitirmek için Enter'a basın veya Kapat'ı seçin ({ $count } nokta, yükseklik { $offset }).
ab-prompt-ceiling-height = Alt yüzün kata göre yüksekliği (şimdi { $offset })
ab-prompt-shaft-start = { $storey } katından şaft: izdüşümün ilk noktasını belirtin.
ab-prompt-shaft-next = { $storey } katından şaft: sonraki noktayı belirtin ya da bitirmek için Enter'a basın veya Kapat'ı seçin ({ $count } nokta).
ab-prompt-cut-start = { $floor } içinde boşluk: kesimin ilk noktasını belirtin.
ab-prompt-cut-next = { $floor } içinde boşluk: sonraki noktayı belirtin ya da bitirmek için Enter'a basın veya Kapat'ı seçin ({ $count } nokta).
ab-prompt-opening-place = { $wall } içinde boşluk: bir duvara tıklayın ya da mesafe yazın ({ $width } x { $height })
ab-prompt-opening-rise = Kemer yüksekliği (dikdörtgen için 0)
ab-keyword-opening-rise = Kemer yüksekliği
ab-type-ceiling-generic = Genel tavan 20
ab-type-ceiling-grid = Asma ızgara 600 x 600
ab-type-ceiling-generated = Tavan { $mm }
ab-material-ceiling-board = Tavan levhası
ab-material-ceiling-tile = Tavan karosu
ab-prompt-arc-mid = Yay: yay üzerinde bir nokta belirtin.
ab-prompt-arc-end = Yay: yayın bitiş noktasını belirtin.
ab-keyword-arc = Yay
ab-prompt-shaft-base = Taban kotu (adı)
ab-prompt-shaft-top = Tepe kotu (adı)
ab-prompt-shaft-offset = Tepe kotundan öteleme
ab-keyword-shaft-base = Taban kotu
ab-keyword-shaft-top = Tepe kotu
ab-keyword-shaft-offset = Tepe ötelemesi

ab-cmd-last-view = Bir katın son plan görünümü silinemez. Bir katta en az bir görünüm bulunur.
ab-cmd-view-duplicate-label = Plan Görünümünü Çoğalt
ab-cmd-view-duplicate-description = Bir plan görünümünü aralığı, ölçeği, ayrıntı düzeyi, kategori işaretleri ve gizlenen öğeleriyle yeni bir adla kopyalar.
ab-cmd-view-rename-label = Plan Görünümünü Yeniden Adlandır
ab-cmd-view-rename-description = Bir plan görünümünü yeniden adlandırır; ad, aynı katın diğer görünümlerinde kullanılmamış olmalıdır.
ab-cmd-view-delete-label = Plan Görünümünü Sil
ab-cmd-view-delete-description = Bir plan görünümünü siler. Bir katın son görünümü kalır.
ab-cmd-view-hide-label = Görünümde Gizle
ab-cmd-view-hide-description = Öğeleri yalnızca bir plan görünümünde gizler; model ve diğer görünümler değişmez.
ab-cmd-view-show-label = Görünümde Göster
ab-cmd-view-show-description = Bir plan görünümünde gizlenen öğeleri yeniden gösterir; öğe verilmezse hepsini gösterir.
ab-cmd-view-defaults-label = Varsayılan Plan Görünümlerini Oluştur
ab-cmd-view-defaults-description = Plan görünümü olmayan her kata varsayılan görünümünü verir.
ab-cmd-view-set-label = Görünüm Ayarını Değiştir
ab-cmd-view-set-description = Bir plan görünümünün tek bir ayarını değiştirir; görünümü olmayan bir kat için önce görünümünü oluşturur.

ab-cmd-set-create-label = Seçim Kümesi Oluştur
ab-cmd-set-create-description = Belgede adlandırılmış bir seçim kümesi kaydeder: verilen öğeler, yoksa seçili öğeler.
ab-cmd-set-update-label = Seçim Kümesini Değiştir
ab-cmd-set-update-description = Bir seçim kümesine öğe ekler veya üyelerini çıkarır.
ab-cmd-set-rename-label = Seçim Kümesini Yeniden Adlandır
ab-cmd-set-rename-description = Bir seçim kümesini yeniden adlandırır; ad diğer kümelerce kullanılmıyor olmalıdır.
ab-cmd-set-delete-label = Seçim Kümesini Sil
ab-cmd-set-delete-description = Bir seçim kümesini siler; öğeleri modelde kalır.
ab-cmd-set-select-label = Küme Üyelerini Seç
ab-cmd-set-select-description = Bir seçim kümesinin üyelerini seçer; seçimin yerine geçer veya ona eklenir.
ab-cmd-statistics-label = Model İstatistikleri
ab-cmd-statistics-description = Öğe sayılarını tür, tip ve kata göre; ayrıca görünüm, seçim kümesi ve öğe tipi sayısını bildirir.
ab-cmd-floor-slope-label = Döşeme eğimi
ab-cmd-floor-slope-description = Döşemeye bir eğim oku verir: bir başlangıç çizgisi, bir yön ve bir eğim ya da uç yüksekliği. Düz döşemede yoktur.
ab-cmd-floor-slope-edge-label = Kenardan döşeme eğimi
ab-cmd-floor-slope-edge-description = Döşemeyi, üst kotta kalan sınır kenarlarından biri boyunca eğimlendirir.
ab-cmd-floor-slope-points-label = Üç noktayla döşeme eğimi
ab-cmd-floor-slope-points-description = Döşemeyi, üst yüzü verilen üç kottan geçecek biçimde eğimlendirir.
ab-cmd-floor-edge-create-label = Döşeme kenarı
ab-cmd-floor-edge-create-description = Seçilen döşeme kenarları boyunca bir profili süpürür (döşeme kenarı, bordür, damlalık); köşelerde gönyelenir ve kenarlar değişince döşemeyi izler.
ab-cmd-floor-edge-edit-label = Döşeme kenarını düzenle
ab-cmd-floor-edge-edit-description = Bir döşeme kenarının profilini ya da kenarlarını değiştirir.
ab-cmd-roof-extrusion-create-label = Ekstrüzyonla çatı
ab-cmd-roof-extrusion-create-description = Düşey bir düzlemde çizilen açık bir profili bir başlangıç ve bir bitiş uzaklığı arasında ekstrüde ederek çatı oluşturur; türünün katmanları profile dik ötelenir.
ab-cmd-roof-extrusion-edit-label = Ekstrüzyonla çatıyı düzenle
ab-cmd-roof-extrusion-edit-description = Ekstrüzyonla çatının düzlemini, profilini ya da ekstrüzyon uzaklıklarını değiştirir.
ab-cmd-railing-create-label = Korkuluk
ab-cmd-railing-create-description = Çizgi ve yaylardan oluşan bir yol boyunca ya da bir merdiven veya rampaya bağlı olarak, onu izleyen bir korkuluk oluşturur. Tip yüksekliği, tutamağı, balusterleri ve dikmeleri verir; uç uzantıları ve sürekli tutamak korkulukta ayarlanır.
ab-cmd-railing-type-define-label = Korkuluk Tipi
ab-cmd-railing-type-define-description = Bir korkuluk tipi oluşturur ya da adı verileni düzenler: yükseklik, tutamak ve baluster profilleri, baluster aralığı, dikme boyutu, korkuluk yüksekliği ve köşe birleşimi.
ab-cmd-ramp-create-label = Rampa
ab-cmd-ramp-create-description = Eğimli kolun ayağından ve ucundan düz bir rampa oluşturur; sahanlıklar, genişlik, kalınlık ve biçim verilebilir. Eğim yükseklik farkından ve uzunluktan çıkar; tipinin en büyük eğimini aşması bir uyarıdır. Seçilen yanlara korkuluk yapılır; üstteki kata varan rampa o döşemede boşluk açar.
ab-cmd-ramp-type-define-label = Rampa Tipi
ab-cmd-ramp-type-define-description = Bir rampa tipi oluşturur ya da adı verileni düzenler: en büyük eğim ve kalınlık.
ab-prompt-railing-first = Korkuluk: yolun ilk noktasını belirtin.
ab-prompt-railing-next = Korkuluk ({ $count } nokta): sonraki noktayı belirtin ya da bitirmek için Enter'a basın.
ab-keyword-railing-done = Bitir
ab-prompt-ramp-start = Rampa: eğimli kolun ayağını belirtin.
ab-prompt-ramp-end = Rampa: eğimli kolun ucunu belirtin.
ab-cmd-no-work-plane = Çalışma düzlemi için bir kot, aks, referans düzlemi, duvar, döşeme ya da tavan (adını verin veya seçin) ya da aynı doğru üzerinde olmayan üç nokta gerekir.
ab-cmd-no-line = Başlangıç ve bitiş noktalarını ya da çizgisi alınacak bir duvarı verin.
ab-cmd-grid-create-label = Aks
ab-cmd-grid-create-description = İki noktadan, bir duvarın eksenlerinden ya da ardışık duvarlardan düz, yay biçiminde veya bir nokta dizisi boyunca çok parçalı aks çizer. Etiket bir öncekini izler: 1, 2, 3 ya da A, B, C ya da A.1, A.2.
ab-cmd-reference-plane-create-label = Referans düzlemi
ab-cmd-reference-plane-create-description = Sınırlı ya da sınırsız, adlandırılmış düşey bir referans düzlemi yapar; güçlü, zayıf ya da yok olan gücü yakalamanın bulup bulmayacağını belirler.
ab-cmd-work-plane-set-label = Çalışma düzlemi
ab-cmd-work-plane-set-description = Bir kotu, aksı, referans düzlemini, bir duvarın, döşemenin ya da tavanın yüzünü ya da üç noktayı etkin çalışma düzlemi yapar ve gösterir.
ab-cmd-work-plane-show-label = Çalışma düzlemini göster
ab-cmd-work-plane-show-description = Etkin çalışma düzlemini gösterir ya da gizler.
ab-cmd-datum-set-extent-label = Datum uzantısı
ab-cmd-datum-set-extent-description = Bir aksın hangi kotlar arasında uzandığını ve her ucun baş işaretini gösterip göstermediğini ayarlar.
ab-cmd-datum-set-view-extent-label = Görünümlerde aks uzantısı
ab-cmd-datum-set-view-extent-description = Aks uçlarını yalnızca seçilen plan görünümlerinde içeri çeker ya da dışarı iter, baş işaretlerini gösterir veya gizler; bu ayarları bir görünümden diğerlerine kopyalar. Aksın kendisi değişmez.
ab-prompt-grid-start = Aks: başlangıç noktasını belirtin.
ab-prompt-grid-end = Aks: bitiş noktasını belirtin.
ab-prompt-reference-plane-start = Referans düzlemi: ilk noktayı belirtin.
ab-prompt-reference-plane-end = Referans düzlemi: ikinci noktayı belirtin.
ab-type-wall-curtain = Perde Duvar
ab-cmd-curtain-type-label = Perde Duvar Tipi
ab-cmd-curtain-type-description = Perde duvar tipi oluşturur ya da düzenler: iki yöndeki ızgara kuralları, sistem paneli ile ara, kenar ve köşe doğramaları; tipin bütün perde duvarları onunla birlikte değişir.
ab-cmd-curtain-grid-add-label = Izgara Çizgisi Ekle
ab-cmd-curtain-grid-add-description = Perde duvara dikey ya da yatay bir ızgara çizgisi ekler; bütün duvar boyunca veya tek bir parça boyunca.
ab-cmd-curtain-grid-remove-label = Izgara Parçasını Kaldır
ab-cmd-curtain-grid-remove-description = Perde duvarın bir ızgara çizgisini ya da bir parçasını kaldırır; yanındaki gözler birleşir.
ab-cmd-curtain-grid-move-label = Izgara Çizgisini Taşı
ab-cmd-curtain-grid-move-description = Perde duvarın bir ızgara çizgisini taşır ya da olduğu yere sabitler; ızgara kuralı değişse de yerinde kalır.
ab-cmd-curtain-panel-label = Panel Ata
ab-cmd-curtain-panel-description = Perde duvarın bir gözüne camlı, dolu ya da boş panel ya da kapı veya pencere verir; seçimi temizler.
ab-cmd-curtain-wall-label = Perde Duvar
ab-cmd-curtain-wall-description = Bir kata iki noktalı bir eksen çizgisinden perde duvar yerleştirir; tip verilmedikçe belgedeki ilk perde duvar tipini kullanır.

## Döşeme eğimi, ekstrüzyonla çatı ve seçim kümesi komutlarının istemleri
ab-prompt-floor-slope-from = Döşeme eğimi: okun başlangıcını belirtin (döşeme oradaki oka dik çizgi boyunca üst kotunu korur).
ab-prompt-floor-slope-to = Döşeme eğimi: okun ucunu belirtin (döşemenin yükseldiği yön).
ab-prompt-floor-slope-value = Döşeme eğimi: eğimi yazın: açı (2°), yüzde (2%) veya yükselti:yatay (1:50); yalın sayı projenin eğim birimindedir.
ab-prompt-roof-extrusion-plane-start = Ekstrüzyonla çatı: profilin çizildiği düşey düzlemin ilk noktasını belirtin.
ab-prompt-roof-extrusion-plane-end = Ekstrüzyonla çatı: düzlemin yatay ekseni üzerinde ikinci bir nokta belirtin.
ab-prompt-roof-extrusion-profile = Ekstrüzyonla çatı: profilin sonraki noktasını u,z olarak yazın (düzlem ekseni boyunca, kotun üstündeki yükseklik) veya bitirmek için Enter'a basın ({ $count } nokta).
ab-keyword-roof-extrusion-done = Bitti
ab-prompt-roof-extrusion-end = Ekstrüzyonla çatı: ekstrüzyonun düzlemden derinliğini yazın (eksi değer diğer tarafa gider).
ab-prompt-set-name = Yeni seçim kümesinin adı (seçili elemanlar üyeleri olur):
ab-prompt-set-select = Seçilecek seçim kümesinin adı ({ $count } küme; Enter ilkini alır):
ab-cmd-set-unknown = Bu adda bir seçim kümesi yok. Önce bir seçim kümesi kaydedin veya mevcut birinin adını yazın.
ab-cmd-coordinates-set-label = Proje Konumu
ab-cmd-coordinates-set-description = Proje başlangıç noktasını, ölçüm noktasını, gerçek kuzeyi ve jeodezik referansı (koordinat sistemi, doğu, kuzey, yükseklik, ölçek) ayarlar. Hiçbir öğe yer değiştirmez; yalnızca bir nokta için bildirilen koordinatlar değişir. Değer verilmezse projeyi seçer; özellikler panosu aynı alanları listeler.
ab-cmd-coordinates-query-label = Koordinatlar
ab-cmd-coordinates-query-description = Bir noktayı iç, proje, ölçüm ve harita koordinatlarında bildirir.
ab-prompt-coordinates-point = Koordinatlar: bir nokta seçin.
ab-cmd-coordinates-recenter-label = Modeli Merkezle
ab-cmd-coordinates-recenter-description = Tüm modeli proje başlangıç noktası ve ölçüm noktasıyla birlikte, ölçüm noktası iç orijine gelecek biçimde taşır. Her öğenin harita koordinatları aynı kalır.
