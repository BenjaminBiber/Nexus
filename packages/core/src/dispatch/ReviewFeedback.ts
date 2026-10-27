import type { Comment } from '../board/BoardTypes.js';
import { DISPATCH_MARKERS } from './DispatchConfig.js';

// Offene Rückmeldungen aus dem Review: „Zurück an den Agenten“-Kommentare des Dispatchers seit dem letzten Start.
export class ReviewFeedback {
  // Texte der Rückmeldungen nach dem letzten `[dispatch] Start` des Dispatchers, älteste zuerst
  pending(comments: Pick<Comment, 'authorName' | 'body'>[], dispatcherName: string): string[] {
    const own = comments.filter((c) => c.authorName === dispatcherName);
    const lastStart = own.map((c) => c.body.startsWith(DISPATCH_MARKERS.START)).lastIndexOf(true);
    return own
      .slice(lastStart + 1)
      .filter((c) => c.body.startsWith(DISPATCH_MARKERS.REVIEW_RETURN))
      .map((c) => c.body.slice(DISPATCH_MARKERS.REVIEW_RETURN.length).replace(/^\s*(\([^)]*\))?\s*:\s*/, '').trim())
      .filter(Boolean);
  }
}
