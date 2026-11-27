# archbuilder-io iletileri: dışa aktarma komutu, hataları ve IFC dışa aktarmanın kayıp nedenleri
# (crates/archbuilder-io/src).

ab-io-command-export-ifc-label = IFC…
ab-io-command-export-ifc-description = Belgeyi bir yola IFC4 dosyası olarak ve kayıp raporuyla yazar.
ab-io-no-structure = Belgede dışa aktarılacak proje, arsa ve bina yok.
ab-io-loss-object-not-exported = Bir nesne okunamadı ve IFC dosyasında yok.
ab-io-loss-body-not-exported = Bir öğe IFC dosyasında şekli olmadan yer alıyor.
ab-io-loss-quantity-not-exported = Bir öğenin bir miktarı IFC dosyasına yazılmadı.
ab-io-loss-window-operation = Menteşeli, üstten ya da alttan menteşeli, pivot ya da katlanır pencere tek ya da çift panel olarak yazıldı: IFC4'te pencerenin kendisinde açılma bilgisi yok, bu yüzden açılma türü, menteşe tarafı ve yönü dosyada yer almıyor.
ab-io-path-required = Dosya adı verilmedi. Komut satırına archbuilder.file.export_ifc path=<dosya> yazın.
ab-io-loss-room-unenclosed = Bir oda IFC dosyasında yok: onu çevreleyen kapalı bir duvar halkası olmadığından mekan olarak yazılacak bir bölgesi yok.
ab-io-loss-room-redundant = Bir oda IFC dosyasında yok: bölgesi başka bir odada olduğundan ve her bölge tek bir mekan olarak yazıldığından.
ab-io-command-export-shortcuts-label = Kısayollar…
ab-io-command-export-shortcuts-description = Yazılan komut kısayol tablosunu paylaşmak ya da geri yüklemek için bir dosyaya yazar.
ab-io-command-import-shortcuts-label = Kısayollar…
ab-io-command-import-shortcuts-description = Yazılan komut kısayol tablosunu bir dosyadan okur ve kullanır.
ab-io-loss-arc-faceted = Bir yay kenarı veya kemerli üst, iki milimetre içinde düz kirişler olarak yazıldı: buradaki IFC gövdeleri çokyüzlü.
ab-io-loss-cut-not-applied = Bir şaft veya düşey boşluk, geçtiği elemanın gövdesinden kesilemedi: gövde bütün olarak yazıldı, boşluk dosyada elemanın bir boşluğu olarak yer alıyor.
ab-io-validation-unreadable = IFC dosyası yazıldı ancak geri okunamadı; dosya eksiktir.
ab-io-validation-missing = Modeldeki bir nesne IFC dosyasında kendi tanımlayıcısıyla bulunmuyor; dosya eksiktir.
ab-io-validation-duplicate-id = IFC dosyasındaki iki varlığın genel tanımlayıcısı aynı; dosya eksiktir.
ab-io-validation-count = IFC dosyasında { $found } öğe var ancak dışa aktarım { $expected } öğe saymıştı; dosya eksiktir.
ab-io-loss-floor-edge-as-covering = IFC4'te döşeme kenarı için bir eleman yok: döşeme kenarı, süpürülmüş gövdesiyle pervaz türünde bir kaplama olarak yazıldı.
ab-io-loss-floor-edge-orphan = Bir döşeme kenarı, döşemesinde artık bulunmayan kenarları adlandırıyor: bu kenarlar dosyada yok.
ab-io-loss-datum-not-written = Bir referans düzlemi ya da çalışma düzlemi IFC dosyasında yok: yapı elemanı değil, bir çizim yardımcısı.
ab-io-loss-grid-one-direction = Bir aks çizgisi IFC dosyasında yok: IFC aksı iki yönde çizgi ister, bu yapının aksları tek yönde.
ab-io-loss-curtain-panel-filled = Bir perde duvarın panelleri arasında { $count } kapı veya pencere var: her biri perde duvarın parçası olarak değil, perde duvarda açılan bir boşluğu dolduran kapı veya pencere olarak yazıldı.
ab-io-loss-dormer-as-parts = IFC4'te çatı penceresi için bir eleman yok: çatısındaki açıklık, ön ve yan duvarlarının her biri için bir duvar ve kendi küçük çatısı için bir çatı olarak yazıldı.
ab-io-loss-fascia-as-covering = IFC4'te saçak silmesi için bir eleman yok: süpürülmüş gövdesiyle pervaz türünde bir kaplama olarak yazıldı.
ab-io-loss-roof-edge-orphan = Bir çatı kenarı elemanı, çatısında artık bulunmayan ya da eğimi olmayan kenarları adlandırıyor: bu kenarlar dosyada yok.
ab-io-loss-project-info-not-written = Proje bilgisi IFC dosyasına eklenemedi: uzun ad, aşama, adresler, aktörler ve yapı özellikleri dışarıda kaldı.
ab-io-loss-issue-date-not-written = Projenin yayın tarihi IFC dosyasında yok: IFC4’te bunun için bir öznitelik bulunmaz.
ab-io-command-new-from-template-label = Şablondan Yeni
ab-io-command-new-from-template-description = Açık belgenin projesini, sıradan bir belge olan bir şablondan başlatır: proje ayarları ve bilgisi, saha, yapı, katlar, seviyeler, eleman tipleri ve plan görünümleri yeni kimliklerle kopyalanır ve belgenin mevcut yapısının yerini alır. Şablon değişmez. Belgede eleman bulunmamalıdır.
ab-io-command-transfer-standards-label = Standartları Aktar
ab-io-command-transfer-standards-description = Eleman tiplerini ve görüntüleme birimi ayarlarını başka bir belge dosyasından kopyalar. Belgede adı zaten bulunan tip, tipe göre korunur, üzerine yazılır (hiçbir şey kullanmıyorsa) ya da boş bir adla kopyalanır; sonuç neyin kopyalandığını ve neyin dışarıda kaldığını listeler.
ab-io-loss-selection-set-member-not-written = Bir seçim kümesinin { $count } üyesi IFC dosyasında yok, çünkü elemanları dosyada değil; grup diğerlerini içerir.
ab-io-validation-conformance = IFC dosyası bildirdiği şemaya uymuyor: { $entity } #{ $instance } üzerinde { $rule } ({ $attribute }), bu yüzden dosya eksiktir.
