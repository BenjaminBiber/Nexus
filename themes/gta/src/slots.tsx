// Slots außer der Shell: je Stil eigene Bausteine. NavIcon, Icon und Progress folgen dem aktiven Stil (ohne Theme-Host,
// auf Seiten ohne Projekt, dem gemerkten); die Startanimation läuft vor dem Host und nimmt den gemerkten Stil.
import { useState, type ComponentType } from 'react';
import type { IconName, NavIconName, ProgressProps, StartAnimationProps } from '@nexus/theme-sdk';
import { rememberedStyle, useSlotStyle, type GtaSettings } from './settings.js';
import { SaBoot } from './sa/SaBoot.js';
import { SaIcon } from './sa/SaIcon.js';
import { SaNavIcon } from './sa/SaNavIcon.js';
import { SaProgress } from './sa/SaProgress.js';
import { VBoot } from './v/VBoot.js';
import { VIcon } from './v/VIcon.js';
import { VNavIcon } from './v/VNavIcon.js';
import { VProgress } from './v/VProgress.js';

interface StyleSlots {
  NavIcon: ComponentType<{ name: NavIconName }>;
  Icon: ComponentType<{ name: IconName }>;
  Progress: ComponentType<ProgressProps>;
  StartAnimation: ComponentType<StartAnimationProps>;
}

/** Bausteine je Stil */
export const STYLE_SLOTS: Record<GtaSettings['style'], StyleSlots> = {
  v: { NavIcon: VNavIcon, Icon: VIcon, Progress: VProgress, StartAnimation: VBoot },
  sa: { NavIcon: SaNavIcon, Icon: SaIcon, Progress: SaProgress, StartAnimation: SaBoot },
};

export function GtaNavIcon({ name }: { name: NavIconName }) {
  const { NavIcon } = STYLE_SLOTS[useSlotStyle()];
  return <NavIcon name={name} />;
}

export function GtaIcon({ name }: { name: IconName }) {
  const { Icon } = STYLE_SLOTS[useSlotStyle()];
  return <Icon name={name} />;
}

export function GtaProgress(props: ProgressProps) {
  const { Progress } = STYLE_SLOTS[useSlotStyle()];
  return <Progress {...props} />;
}

export function GtaStartAnimation(props: StartAnimationProps) {
  const [style] = useState(rememberedStyle);
  const { StartAnimation } = STYLE_SLOTS[style];
  return <StartAnimation {...props} />;
}
