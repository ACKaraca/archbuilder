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
       *[other] eksen
    })
ab-prompt-wall-end = { $storey } üzerinde duvar: sonraki noktayı belirtin ya da bitirmek için Enter'a basın (kalınlık { $thickness }, yükseklik { $height }, çizgi { $location ->
        [exterior] dış yüz
        [interior] iç yüz
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
ab-cmd-floor-no-loop = Bu duvarlar kapalı bir sınır oluşturmuyor. Alanı çevreleyen, uç uca gelen duvarları seçin ya da kapalı bir duvar halkasının içine tıklayın.
ab-cmd-floor-walls-parallel = Sınırda birbirini izleyen iki duvar paralel, kesişmiyorlar. Köşelerde buluşan duvarları seçin.
ab-cmd-no-floor-selected = Adı verilmiş ya da seçili bir döşeme yok. Önce bir döşeme seçin.
ab-cmd-floor-too-big = Bir döşeme en çok { $loops } halka ve toplam { $points } nokta alabilir.
ab-cmd-floor-create-label = Döşeme
ab-cmd-floor-create-description = Bir katta; seçilen duvarlardan (kenarlar duvar yüzlerini izler) ya da çizilen bir sınırdan; kalınlığı ve üst yüzünün kat seviyesine göre kot farkıyla döşeme oluşturur.
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
