// Typen der Telegram-Routen; die Formen kommen aus ts-rs (src/api/generated/telegram.ts).
export type {
  AppliedProfile, BotProfile, BotState, BotStatus, IdeaSettings, IdeaTarget, QuietHours,
  SpeechBackend, SpeechConfig, SpeechTestDto, TelegramConfig, TelegramNotify, TelegramPairingDto,
  TelegramProfileDto, TelegramStatusDto, TelegramTestDto, TelegramTestError, WhisperCli,
} from './generated/telegram.js';

import type { BotProfile, SpeechConfig, TelegramConfig } from './generated/telegram.js';

// Was die Oberfläche speichern darf: die Kopplung (`chats`) und das zuletzt eingerichtete
// Profil (`profile.applied`) schreibt nur der Server und bleiben unberührt. Ein Geheimnis
// (`botToken`, `speech.apiKey`) bleibt beim gespeicherten Wert, solange es fehlt.
export type TelegramConfigInput = Partial<Omit<TelegramConfig, 'chats' | 'profile' | 'speech'>> & {
  profile?: Pick<BotProfile, 'brand'>;
  speech?: Omit<SpeechConfig, 'apiKey'> & { apiKey?: string };
};
