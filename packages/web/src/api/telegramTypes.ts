// Typen der Telegram-Routen; die Formen kommen aus ts-rs (src/api/generated/telegram.ts).
export type {
  AppliedProfile, BotProfile, BotState, BotStatus, IdeaSettings, IdeaTarget, QuietHours,
  SpeechBackend, SpeechConfig, TelegramConfig, TelegramNotify, TelegramPairingDto,
  TelegramProfileDto, TelegramStatusDto, TelegramTestDto, TelegramTestError, WhisperCli,
} from './generated/telegram.js';

import type { BotProfile, TelegramConfig } from './generated/telegram.js';

// Was die Oberfläche speichern darf: die Kopplung (`chats`) und das zuletzt eingerichtete
// Profil (`profile.applied`) schreibt nur der Server und bleiben unberührt.
export type TelegramConfigInput = Partial<Omit<TelegramConfig, 'chats' | 'profile'>> & {
  profile?: Pick<BotProfile, 'brand'>;
};
