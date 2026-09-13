use std::marker::PhantomData;

use anyhow::Result;
use serde::ser::SerializeStruct;
use small_fixed_array::FixedString;

fn deserialize_single_seq<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    struct SingleVisitor<T>(PhantomData<T>);

    impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for SingleVisitor<T> {
        type Value = Option<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a sequence")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            seq.next_element()
        }
    }

    deserializer.deserialize_seq(SingleVisitor(PhantomData))
}

#[derive(serde::Serialize)]
struct TranslateRequest<'a> {
    text: &'a str,
    target_lang: &'a str,
    preserve_formatting: u8,
    formality: &'a str,
    model_type: &'a str,
}

#[derive(serde::Deserialize)]
struct Translation {
    pub text: FixedString,
    pub detected_source_language: FixedString<u8>,
}

#[derive(serde::Deserialize)]
struct TranslateResponse {
    #[serde(deserialize_with = "deserialize_single_seq")]
    pub translations: Option<Translation>,
}

fn auth_header(token: &str) -> String {
    format!("DeepL-Auth-Key {token}")
}

pub async fn run(
    reqwest: &reqwest::Client,
    token: &str,
    content: &str,
    target_lang: &str,
) -> Result<Option<FixedString>> {
    let request = TranslateRequest {
        target_lang,
        text: content,
        preserve_formatting: 1,
        formality: "prefer_less",
        model_type: "latency_optimized",
    };

    let response: TranslateResponse = reqwest
        .get("https://api.deepl.com/v2/translate")
        .query(&request)
        .header("Authorization", auth_header(token))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if let Some(translation) = response.translations
        && translation.detected_source_language != target_lang
    {
        return Ok(Some(translation.text));
    }

    Ok(None)
}

#[derive(serde::Deserialize)]
struct Voice {
    pub name: FixedString<u8>,
    pub lang: FixedString<u8>,
    pub usable_as_target: bool,
}

struct VoiceRequest;
impl serde::Serialize for VoiceRequest {
    fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut serializer = serializer.serialize_struct("VoiceRequest", 1)?;
        serializer.serialize_field("resource", "translate_text")?;
        serializer.end()
    }
}

pub async fn get_languages(
    reqwest: &reqwest::Client,
    token: &str,
) -> Result<Vec<(FixedString<u8>, FixedString<u8>)>> {
    let languages: Vec<Voice> = reqwest
        .get("https://api.deepl.com/v3/languages")
        .query(&VoiceRequest)
        .header("Authorization", auth_header(token))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let language_map = languages
        .into_iter()
        .filter(|v| v.usable_as_target)
        .map(|v| (v.lang, v.name))
        .collect();

    Ok(language_map)
}
