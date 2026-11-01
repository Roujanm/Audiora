# HiNekst — Seslendirme (ElevenLabs)

**Ayarlar:** Türkçe destekleyen model (Eleven Multilingual v2 veya v3), sıcak ve samimi kadın ses, doğal hız (Speed 1.0, asla hızlandırma). Stability ~0.45, Similarity ~0.75, Style düşük. Tüm metin **tek çağrıda**, 2 take; kelimesi kelimesine doğru olanı tut.

## Yapıştırılacak metin (duraklar dahil)

```
Bir merhaba… <break time="0.5s" /> …bir toplulukla başlar. <break time="0.7s" /> Tek dokunuş… <break time="0.3s" /> ve takımın seste. <break time="2.5s" /> Kendi sunucun, kendi kuralların. <break time="0.8s" /> Merhaba'dan… <break time="0.3s" /> Sonrakine. <break time="1.0s" /> HiNekst.
```

`<break>` etiketi modelde çalışmazsa aynı metni, duraklar yerine boş satırlarla yapıştırın:

```
Bir merhaba…

…bir toplulukla başlar.

Tek dokunuş…

ve takımın seste.

Kendi sunucun, kendi kuralların.

Merhaba'dan…

Sonrakine.

HiNekst.
```

## Hedef başlangıç zamanları (video saniyesi)
| Satır | Başlangıç |
|---|---|
| Bir merhaba… | 0.50 |
| …bir toplulukla başlar. | 2.67 |
| Tek dokunuş… | 5.40 |
| ve takımın seste. | 6.60 |
| Kendi sunucun, kendi kuralların. | 11.20 |
| Merhaba'dan… | 13.50 |
| Sonrakine. | 15.10 |
| HiNekst. | 17.50 |

Dosya gelince satırları sessizlikten otomatik keserim ve bu zamanlara yerleştiririm; süre uymazsa zaman çizelgesini sese göre kaydırırım.

**Not:** "HiNekst" okunuşu: "Hay-nekst" mi, "Hi-nekst" mi? Telaffuz sorun çıkarırsa metne fonetik yazım ("Hay Nekst") eklenir; hangisini istediğinizi söyleyin.

Teslim: dosyayı `hinekst-video\assets\voice\` klasörüne koyun (WAV veya MP3) ya da yolunu yazın.
