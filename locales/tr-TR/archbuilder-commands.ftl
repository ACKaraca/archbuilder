# archbuilder-commands iletileri: hata kodları ve komut etiketleri. Tanımlayıcılar locales/en-US/archbuilder-commands.ftl ile aynıdır.

ab-cmd-document-not-empty = Zaten projesi olan bir belgede yeni proje oluşturulamaz.
ab-cmd-kind-mismatch = Bu komut, bu türdeki bir nesne üzerinde çalışmaz.
ab-cmd-no-building = Belgede kat eklenecek bir bina yok.
ab-cmd-nothing-selected = Önce bir kat ya da duvar seçin.
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

ab-cmd-storey-has-walls = Kat silinemez, çünkü üzerinde duvar var (adet: { $count }). Önce duvarları silin.
ab-cmd-wall-end-same = Bitiş noktası başlangıç noktasıyla aynı. Duvarın bir uzunluğu olmalı.
ab-cmd-wall-create-label = Duvar
ab-cmd-wall-create-description = Bir kata, iki noktalı bir eksen çizgisinden; kalınlık, kat seviyesinden başlayan yükseklik ve konum çizgisiyle duvar yerleştirir.
ab-cmd-wall-delete-label = Duvarı Sil
ab-cmd-wall-delete-description = Duvarları siler; duvar belirtilmezse seçili olanları.
ab-cmd-wall-flip-label = Duvarı Çevir
ab-cmd-wall-flip-description = Duvarları çevirir: eksen çizgisi ters çevrilir, dış taraf yer değiştirir; konum çizgisi aynı kalır.
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
