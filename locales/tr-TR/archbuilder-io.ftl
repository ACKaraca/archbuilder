# archbuilder-io iletileri: dışa aktarma komutu, hataları ve IFC dışa aktarmanın kayıp nedenleri
# (crates/archbuilder-io/src).

ab-io-command-export-ifc-label = IFC…
ab-io-command-export-ifc-description = Belgeyi bir yola IFC4 dosyası olarak ve kayıp raporuyla yazar.
ab-io-no-structure = Belgede dışa aktarılacak proje, arsa ve bina yok.
ab-io-loss-object-not-exported = Bir nesne okunamadı ve IFC dosyasında yok.
ab-io-loss-body-not-exported = Bir öğe IFC dosyasında şekli olmadan yer alıyor.
ab-io-loss-quantity-not-exported = Bir öğenin bir miktarı IFC dosyasına yazılmadı.
ab-io-loss-window-operation = Açılır pencere tek panel olarak yazıldı: IFC4'te pencerenin kendisinde açılma bilgisi yok, bu yüzden açılma türü, menteşe tarafı ve yönü dosyada yer almıyor.
ab-io-path-required = Dosya adı verilmedi. Komut satırına archbuilder.file.export_ifc path=<dosya> yazın.
ab-io-loss-room-unenclosed = Bir oda IFC dosyasında yok: onu çevreleyen kapalı bir duvar halkası olmadığından mekan olarak yazılacak bir bölgesi yok.
ab-io-loss-room-redundant = Bir oda IFC dosyasında yok: bölgesi başka bir odada olduğundan ve her bölge tek bir mekan olarak yazıldığından.
ab-io-command-export-shortcuts-label = Kısayollar…
ab-io-command-export-shortcuts-description = Yazılan komut kısayol tablosunu paylaşmak ya da geri yüklemek için bir dosyaya yazar.
ab-io-command-import-shortcuts-label = Kısayollar…
ab-io-command-import-shortcuts-description = Yazılan komut kısayol tablosunu bir dosyadan okur ve kullanır.
ab-io-loss-stair-not-written = Bir merdiven IFC dosyasında yok: merdivenler, kollar ve korkuluklar henüz yazılamıyor. Üstteki döşemede açtığı boşluk dosyada var.
ab-io-loss-arc-faceted = Bir yay kenarı veya kemerli üst, iki milimetre içinde düz kirişler olarak yazıldı: buradaki IFC gövdeleri çokyüzlü.
ab-io-loss-cut-not-applied = Bir şaft veya düşey boşluk, geçtiği elemanın gövdesinden kesilemedi: gövde bütün olarak yazıldı, boşluk dosyada elemanın bir boşluğu olarak yer alıyor.
ab-io-loss-railing-not-written = Bir korkuluk IFC dosyasında yok: korkuluklar henüz yazılamıyor.
ab-io-loss-ramp-not-written = Bir rampa IFC dosyasında yok: rampalar ve rampa kolları henüz yazılamıyor. Üstteki döşemede açtığı boşluk dosyada var.
