import type { ReactElement } from 'react';
import { render } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../../src/api/ApiContext.js';
import type { ApiClient } from '../../src/api/ApiClient.js';
import { PlatformProvider } from '../../src/platform/PlatformContext.js';
import type { Platform } from '../../src/platform/Platform.js';

// Rendert eine Seite unter ihrer Adresse mit gefälschtem ApiClient; ohne `platform` die der Umgebung (Browser)
export function renderWithApp(element: ReactElement, options: { api: ApiClient; path: string; route: string; platform?: Platform }) {
  return render(
    <ApiProvider api={options.api}>
      <PlatformProvider value={options.platform}>
        <MemoryRouter initialEntries={[options.path]}>
          <Routes>
            <Route path={options.route} element={element} />
            <Route path="*" element={<div>Andere Seite</div>} />
          </Routes>
        </MemoryRouter>
      </PlatformProvider>
    </ApiProvider>,
  );
}
