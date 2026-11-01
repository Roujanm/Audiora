# HiNekst — "Hi'dan Next'e" (tanıtım filmi briefi)

Kaynak: "Amazon – From A to Z" prompt'u, HiNekst'e uyarlandı. 21.2 sn, 60 fps, 16:9 (1920×1080) + gerçek 9:16 (1080×1920) yeniden yerleşim. Tamamı kodla render edilir (numpy / Pillow / OpenCV), AI video yok. Tek plan, sert kesme yok.

## Fikir
Marka adı zaten "Hi" + "Next". Tek bir amber ışık çizgisi "Merhaba"dan "Sonraki"ne uzanan bir ok olur ve her işi o yapar: logonun altına çarpar, mesaj kutusunun odak çizgisi olur, ses kanalındaki konuşma halkasına akar, toplantı ızgarasındaki kartları birbirine bağlar, sunucu sihirbazında ilerleme çizgisi olur, sloganın altında "Hi → Next" okuna dönüşür ve son vuruşta logoya oturur.

## Görünüm
- Sahne: yakın siyah (#070707), hafif nefes alan sıcak amber ışık, köşede çok hafif lacivert, ince grain.
- Tek vurgu: amber (mağazadaki "Canlı Önizle" düğmesinin sarı-turuncusu, ~#FFB400). Satır başına en fazla bir kelime.
- Arayüz: ekran görüntülerindeki koyu tema (#0B0B0F zemin, kartlar ~#17171C). Baykuş avatarları (Sen kırmızı, Ece mor, Deniz yeşil, Mert turkuaz, Kaan limon, Zeynep sarı) kahramanlar.
- Tipografi: kalın, geniş başlık fontu (SF Pro kullanılamıyor; Segoe UI Bold veya benzeri, aşağıda not).

## Sahneler (saniye, 60 fps)
| # | Zaman | Ne olur | Kaynak görsel |
|---|---|---|---|
| S1 | 0.00–1.60 | Slam: ışık çizgisi derin uzaydan "Hİ" üzerine çarpar (0.17), sarsıntı, kıvılcımlar; çizgi a→z koşup HİNEKST yazısını açar. | banner'daki HİNEKST yazısı + baykuş |
| S2 | 1.60–4.80 | Yazı yukarı çıkar. Telefon iner (2.40). Çizgi mesaj kutusunun odak çizgisi olur; "Turnuva bu akşam 22:00'de" yazılır. Mesajlar 3B derinlikte açılır. | mobil flow 00–02, chat_squad |
| S3 | 4.80–7.20 | Tek dokunuş: Gönder (6.03), mesaj düşer, çizgi ekrana dalar; kamera ekranın içinden geçer. | mobil flow 02, chat_squad |
| S4 | 7.20–10.00 | Drop (7.20): Lobi / Takım 1 ses kanalı. Çizgi baykuşları tek tek bağlar, konuşma halkası yanar; ardından Toplantı ızgarası 3B açılır. | chat_squad desktop, meet |
| S5 | 10.00–13.30 | Sunucu sihirbazı: şablon seçilir (Oyun), önizleme penceresi kalkar; çizgi ilerleme çizgisi olur. "Kendi sunucun" (12.00 oturur). Puan Dükkanı kartı yanında belirir. | guild_create, store |
| S6 | 13.30–16.50 | "Merhaba'dan…" harfleri avatar/ekran dokusuyla dolar, beyaza oturur, "Next" amber. "Hi'dan Next'e." olarak yeniden dizilir; ok Hi'nın altından Next'in altına uzanır (15.20–16.00). | tipografi |
| S7 | 16.50–19.20 | Nefes (müzik kesilir). 16.80'de ok logoya oturur, wordmark yeniden açılır. Logo canlı kalır, altından yavaş ışık geçer. "HiNekst." Odak kayar, ok ucu tek noktaya iner. | logo |
| S8 | 19.20–21.20 | Bitiş kartı: [imza] + küçük "BETA" notu. | — |

## Türkçe seslendirme (ElevenLabs, tek çağrı, kısa duraklarla, 2 take)
| Zaman | Satır |
|---|---|
| 0.50 | "Bir merhaba…" |
| 2.67 | "…bir toplulukla başlar." |
| 5.40 | "Tek dokunuş…" |
| 6.60 | "ve takımın seste." |
| 11.20 | "Kendi sunucun, kendi kuralların." |
| 13.50 | "Merhaba'dan…" |
| 15.10 | "Sonrakine." |
| 17.50 | "HiNekst." |

Ses: sıcak, samimi, doğal hız (asla hızlandırılmaz). Satırlar bu başlangıç zamanlarına yerleştirilir; ses geldikten sonra zaman çizelgesi sese göre kilitlenir.

## Müzik ve ses
- Kodla üretilen müzik: 100 BPM (bir ölçü = 2.4 sn), Fa majör. Frame 0'dan klavye, pluck, sub, pad; drop'tan (7.20) kick/clap/hat; 12.00'de lift; 16.50'de sadece ters swell; 16.80'de tam akor + alçak vuruş.
- ~90 efekt: slam, yazılan her harfe tık, kart swipe'ları, Gönder tıkı + küçük başarı çanı, whoosh'lar, bildirim pop'u, Hi→Next oku için swipe.
- Mix: ses ≈ –16 LUFS, müzik her kelimenin ≥12 dB altında, master –14 LUFS / –1.25 dBTP. 16.80'in ±60 ms'sinde sadece hero vuruşu.

## Marka kuralları
- Logo: sadece taşı, ölçekle, soldur, maskele; yeniden çizme/yeniden renklendirme yok, durduğunda piksel-tam, üstüne bir şey çizilmez. Şu an elimizde resmi logo dosyası yok; banner'daki yazı geçici.
- Işık çizgisi bizim çizimimiz.
- Ekran görüntülerindeki kişi adları (Sen, Ece, Deniz, Mert, Kaan, Zeynep) örnek kullanıcı olarak kalır; gerçek hesap yok.

## Bilinen sorunlar / riskler
- `flow/03.png` ve `full.png` (mobil): "Missing authorization header" hata bildirimi ve "Gönderilmedi" var. Bunlar kullanılmaz; Gönder anı kodla yeniden çizilir.
- Masaüstü görüntüleri 2400×1350; 9:16 için masaüstü kareleri yeniden yerleştirilecek, mobil kareler 1290×2796.
- SF Pro yok; font kararı bekliyor.

## Ortam
`hinekst-video\.venv` (numpy 2.5, Pillow 13, OpenCV 5, imageio-ffmpeg 7.1). Sisteme kurulum yapılmadı.
