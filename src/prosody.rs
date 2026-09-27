#![allow(dead_code)]

/// Intelligent speech prosody, punctuation and natural cadence enhancer
pub struct ProsodyEnhancer;

impl ProsodyEnhancer {
    /// Format raw transcribed text to sound like human speech with natural intonation
    pub fn enhance(text: &str, is_russian: bool) -> String {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        // 1. Capitalize first letter
        let mut chars = trimmed.chars();
        let mut result = match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => return String::new(),
        };

        // 2. Add commas before conjunctions for natural human breathing pauses
        if is_russian {
            let conjunctions = [" но ", " а ", " чтобы ", " потому что ", " если ", " хотя "];
            for conj in conjunctions {
                let comma_conj = format!(",{}", conj);
                if !result.contains(&comma_conj) {
                    result = result.replace(conj, &comma_conj);
                }
            }
        } else {
            let conjunctions = [" but ", " because ", " although ", " however "];
            for conj in conjunctions {
                let comma_conj = format!(",{}", conj);
                if !result.contains(&comma_conj) {
                    result = result.replace(conj, &comma_conj);
                }
            }
        }

        // 3. Detect questions and exclamations if no end punctuation exists
        let last_char = result.chars().last().unwrap_or('.');
        if !['.', '!', '?', ';', ':'].contains(&last_char) {
            let lower = result.to_lowercase();

            let is_question = if is_russian {
                lower.starts_with("где")
                    || lower.starts_with("куда")
                    || lower.starts_with("когда")
                    || lower.starts_with("кто")
                    || lower.starts_with("как")
                    || lower.starts_with("почему")
                    || lower.starts_with("зачем")
                    || lower.starts_with("сколько")
                    || lower.starts_with("можешь")
                    || lower.starts_with("можете")
                    || lower.contains(" да?")
                    || lower.contains(" правда?")
            } else {
                lower.starts_with("where")
                    || lower.starts_with("when")
                    || lower.starts_with("who")
                    || lower.starts_with("what")
                    || lower.starts_with("why")
                    || lower.starts_with("how")
                    || lower.starts_with("is ")
                    || lower.starts_with("are ")
                    || lower.starts_with("can ")
                    || lower.starts_with("could ")
                    || lower.starts_with("do ")
                    || lower.starts_with("does ")
                    || lower.starts_with("did ")
                    || lower.starts_with("will ")
                    || lower.starts_with("would ")
            };

            let is_alert = if is_russian {
                lower.contains("стой")
                    || lower.contains("назад")
                    || lower.contains("сзади")
                    || lower.contains("враг")
                    || lower.contains("граната")
                    || lower.contains("помогите")
                    || lower.contains("беги")
                    || lower.contains("огонь")
            } else {
                lower.contains("watch out")
                    || lower.contains("look out")
                    || lower.contains("behind")
                    || lower.contains("grenade")
                    || lower.contains("help")
                    || lower.contains("enemy")
                    || lower.contains("run")
                    || lower.contains("fire")
                    || lower.contains("stop")
            };

            if is_question {
                result.push('?');
            } else if is_alert {
                result.push('!');
            } else {
                result.push('.');
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prosody_questions() {
        let q_ru = ProsodyEnhancer::enhance("где вражеский танк", true);
        assert_eq!(q_ru, "Где вражеский танк?");

        let q_en = ProsodyEnhancer::enhance("where is the enemy tank", false);
        assert_eq!(q_en, "Where is the enemy tank?");
    }

    #[test]
    fn test_prosody_alerts() {
        let a_ru = ProsodyEnhancer::enhance("осторожно граната", true);
        assert_eq!(a_ru, "Осторожно граната!");

        let a_en = ProsodyEnhancer::enhance("watch out behind you", false);
        assert_eq!(a_en, "Watch out behind you!");
    }

    #[test]
    fn test_prosody_conjunctions() {
        let c_ru = ProsodyEnhancer::enhance("мы отступаем потому что мало патронов", true);
        assert_eq!(c_ru, "Мы отступаем, потому что мало патронов.");
    }
}

