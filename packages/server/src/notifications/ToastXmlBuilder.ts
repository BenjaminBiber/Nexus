import { join } from 'node:path';
import type { ToastModel, ToastOptions } from './ToastModel.js';

// Baut das XML eines Windows-Toasts aus dem Modell (Aufbau laut docs/design/toast/toast-spec.md).
export class ToastXmlBuilder {
  // Vollständiges Toast-XML
  build(model: ToastModel, options: ToastOptions): string {
    const urgent = model.kind === 'wartet' || model.kind === 'gesammelt';
    const sound = urgent ? 'Notification.Reminder' : 'Notification.Default';
    return (
      `<toast launch="${this.attr(model.openUrl)}" activationType="protocol">` +
      `<visual><binding template="ToastGeneric">${this.images(model, options)}` +
      `<text>${this.text(model.title)}</text><text>${this.text(model.line1)}</text><text>${this.text(model.line2)}</text>` +
      `</binding></visual><actions>${this.actions(model)}</actions>` +
      `<audio src="ms-winsoundevent:${sound}"/></toast>`
    );
  }

  // Logo je Zustand, Hero nur bei „wartet“ und eingeschalteter Einstellung
  private images(model: ToastModel, options: ToastOptions): string {
    const logo = this.logo(model);
    let xml = `<image placement="appLogoOverride" src="${this.attr(join(options.assetsDir, logo))}"/>`;
    if (model.kind === 'wartet' && options.heroImages) {
      xml += `<image placement="hero" src="${this.attr(join(options.assetsDir, 'hero-wartet@2x.png'))}"/>`;
    }
    return xml;
  }

  // Dateiname des Logos je Art der Meldung
  private logo(model: ToastModel): string {
    if (model.kind === 'bereit') return 'logo-bereit.png';
    if (model.kind === 'frage-neu' || model.kind === 'recherche') return 'logo-frage.png';
    return 'logo-wartet.png';
  }

  // Einzige Schaltfläche: Board öffnen (bei Fragen „Frage öffnen“, ohne Ticket „Sessions öffnen“); die Meldung schließt sich selbst
  private actions(model: ToastModel): string {
    const isQuestion = model.kind === 'frage-neu' || model.kind === 'recherche';
    const opensSessions = /\/sessions\/?$/.test(model.openUrl);
    const openText = isQuestion ? 'Frage öffnen' : opensSessions ? 'Sessions öffnen' : 'Board öffnen';
    return `<action content="${openText}" arguments="${this.attr(model.openUrl)}" activationType="protocol"/>`;
  }

  // Text für Elementinhalt
  private text(value: string): string {
    return value.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // Text für Attributwerte
  private attr(value: string): string {
    return this.text(value).replace(/"/g, '&quot;');
  }
}
