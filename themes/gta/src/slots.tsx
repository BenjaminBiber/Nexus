// Slots außer der Shell: je Stil eigene Bausteine. NavIcon, Icon und Progress laufen im Host und folgen dem aktiven Stil;
// die Startanimation läuft vor dem Host und nimmt den gemerkten Stil.
import { useState, type ComponentType } from 'react';
import type { IconName, NavIconName, ProgressProps, StartAnimationProps } from '@nexus/theme-sdk';
import { rememberedStyle, useActiveStyle, type GtaSettings } from './settings.js';
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

const V: StyleSlots = { NavIcon: VNavIcon, Icon: VIcon, Progress: VProgress, StartAnimation: VBoot };

/** Bausteine je Stil; bis San Andreas eigene hat, liefern beide Stile die von GTA V */
export const STYLE_SLOTS: Record<GtaSettings['style'], StyleSlots> = { v: V, sa: V };

export function GtaNavIcon({ name }: { name: NavIconName }) {
  const { NavIcon } = STYLE_SLOTS[useActiveStyle()];
  return <NavIcon name={name} />;
}

export function GtaIcon({ name }: { name: IconName }) {
  const { Icon } = STYLE_SLOTS[useActiveStyle()];
  return <Icon name={name} />;
}

export function GtaProgress(props: ProgressProps) {
  const { Progress } = STYLE_SLOTS[useActiveStyle()];
  return <Progress {...props} />;
}

export function GtaStartAnimation(props: StartAnimationProps) {
  const [style] = useState(rememberedStyle);
  const { StartAnimation } = STYLE_SLOTS[style];
  return <StartAnimation {...props} />;
}
