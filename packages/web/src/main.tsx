import '@fontsource/geist-sans/400.css';
import '@fontsource/geist-sans/500.css';
import '@fontsource/geist-sans/600.css';
import '@fontsource/geist-mono/400.css';
import '@fontsource/geist-mono/500.css';
// Material Icons liegen im Build statt bei Google Fonts: kein blockierender Abruf von außen beim Start
import '@fontsource/material-icons/400.css';
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
