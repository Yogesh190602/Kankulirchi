import { Navigate, Route, Routes } from "react-router-dom";

import { AppLayout } from "./components/layout/AppLayout";
import { useBackendEvents } from "./hooks/useBackendEvents";
import { CollectionDetailPage } from "./pages/CollectionDetailPage";
import { CollectionsPage } from "./pages/CollectionsPage";
import { FavoritesPage } from "./pages/FavoritesPage";
import { HistoryPage } from "./pages/HistoryPage";
import { HomePage } from "./pages/HomePage";
import { SchedulerPage } from "./pages/SchedulerPage";
import { SearchPage } from "./pages/SearchPage";
import { SettingsPage } from "./pages/SettingsPage";

export default function App() {
  useBackendEvents();

  return (
    <Routes>
      <Route element={<AppLayout />}>
        <Route index element={<HomePage />} />
        <Route path="search" element={<SearchPage />} />
        <Route path="collections" element={<CollectionsPage />} />
        <Route path="collections/:id" element={<CollectionDetailPage />} />
        <Route path="favorites" element={<FavoritesPage />} />
        <Route path="history" element={<HistoryPage />} />
        <Route path="scheduler" element={<SchedulerPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
