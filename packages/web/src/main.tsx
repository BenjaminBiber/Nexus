// Nur die Zeichensätze, die die Oberfläche braucht: latin und – soweit das Paket ihn anbietet –
// latin-ext. Die Sammel-CSS je Schnitt würde auch Kyrillisch und Griechisch mitliefern.
// geist-sans bringt ausschließlich latin mit, deshalb steht dort kein latin-ext.
import '@fontsource/geist-sans/latin-400.css';
import '@fontsource/geist-sans/latin-500.css';
import '@fontsource/geist-sans/latin-600.css';
import '@fontsource/geist-mono/latin-400.css';
import '@fontsource/geist-mono/latin-ext-400.css';
import '@fontsource/geist-mono/latin-500.css';
import '@fontsource/geist-mono/latin-ext-500.css';
// Material Icons liegen im Build statt bei Google Fonts: kein blockierender Abruf von außen beim Start
import '@fontsource/material-icons/latin-400.css';
import './styles/variables.css';
import './styles/base.css';
import './styles/docs.css';
import './styles/questions.css';
import './styles/dbboard.css';
import './styles/dashboard.css';
import './styles/inbox.css';
import './styles/settings.css';
import './styles/themes.css';
import './styles/win95.css';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { ApiClient } from './api/ApiClient.js';
import { ApiProvider } from './api/ApiContext.js';
import { App } from './App.js';
import { PlatformProvider } from './platform/PlatformContext.js';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ApiProvider api={new ApiClient()}>
      <PlatformProvider>
        <BrowserRouter>
          <App />
        </BrowserRouter>
      </PlatformProvider>
    </ApiProvider>
  </StrictMode>,
);
