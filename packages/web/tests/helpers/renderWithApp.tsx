import type { ReactElement } from 'react';
import { render } from '@testing-library/react';
import { MemoryRouter, Route, Routes } from 'react-router-dom';
import { ApiProvider } from '../../src/api/ApiContext.js';
import type { ApiClient } from '../../src/api/ApiClient.js';

// Rendert eine Seite unter ihrer Adresse mit gefälschtem ApiClient
export function renderWithApp(element: ReactElement, options: { api: ApiClient; path: string; route: string }) {
  return render(
    <ApiProvider api={options.api}>
      <MemoryRouter initialEntries={[options.path]}>
        <Routes>
          <Route path={options.route} element={element} />
          <Route path="*" element={<div>Andere Seite</div>} />
        </Routes>
      </MemoryRouter>
    </ApiProvider>,
  );
}
