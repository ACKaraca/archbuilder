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
ab-model-insert-operation = "{ $name }" { $operation } olarak çalışamaz: kapı açılır, pencere sabit ya da menteşelidir.
ab-model-insert-frame = "{ $name }" kasası net açıklık bırakmıyor. Açıklığı büyütün ya da kasa çubuklarını daraltın.
ab-model-insert-meets-wall = "{ $name }", bulunduğu duvarla orada birleşen ya da kesişen "{ $other }" duvarının içine açılırdı. O duvardan uzağa taşıyın.
