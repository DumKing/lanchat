import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { api } from "../services/tauri-api";
import type {
  DesktopPetPackage,
  DesktopPetRegistrySnapshot,
  DesktopPetSettings,
  PetStatePlaybackConfig,
} from "../types/desktop-pet";

export const useDesktopPetStore = defineStore("desktop-pet", () => {
  const registry = ref<DesktopPetRegistrySnapshot>({ packages: [], issues: [] });
  const settings = ref<DesktopPetSettings | null>(null);
  const loading = ref(false);
  const error = ref("");

  const packages = computed(() => registry.value.packages);
  const issues = computed(() => registry.value.issues);
  const selectedPackage = computed(() =>
    packages.value.find((item) => item.manifest.id === settings.value?.selectedPetId) ?? null,
  );

  function applySnapshot(snapshot: DesktopPetRegistrySnapshot) {
    registry.value = snapshot;
  }

  async function initialize() {
    loading.value = true;
    error.value = "";
    try {
      const [snapshot, nextSettings] = await Promise.all([
        api.listDesktopPets(),
        api.getDesktopPetSettings(),
      ]);
      registry.value = snapshot;
      settings.value = nextSettings;
    } catch (reason) {
      error.value = String(reason);
    } finally {
      loading.value = false;
    }
  }

  async function refresh() {
    loading.value = true;
    try {
      registry.value = await api.refreshDesktopPets();
      error.value = "";
    } catch (reason) {
      error.value = String(reason);
    } finally {
      loading.value = false;
    }
  }

  async function importPackage(sourcePath: string) {
    loading.value = true;
    try {
      const imported = await api.importDesktopPet(sourcePath);
      registry.value = await api.refreshDesktopPets();
      settings.value = await api.selectDesktopPet(imported.manifest.id);
      error.value = "";
      return imported;
    } catch (reason) {
      error.value = String(reason);
      throw reason;
    } finally {
      loading.value = false;
    }
  }

  async function selectPackage(petId: string) {
    settings.value = await api.selectDesktopPet(petId);
  }

  async function removePackage(pet: DesktopPetPackage) {
    await api.removeDesktopPet(pet.manifest.id);
    registry.value = await api.refreshDesktopPets();
    settings.value = await api.getDesktopPetSettings();
  }

  async function updateSettings(next: DesktopPetSettings) {
    settings.value = await api.updateDesktopPetSettings(next);
  }

  async function refreshSettings() {
    settings.value = await api.getDesktopPetSettings();
  }

  async function updatePlaybackConfig(petId: string, configs: Record<string, PetStatePlaybackConfig>) {
    await api.updateDesktopPetPlaybackConfig(petId, configs);
    registry.value = await api.refreshDesktopPets();
  }

  async function setEnabled(enabled: boolean) {
    await api.setDesktopPetEnabled(enabled);
    if (settings.value) settings.value = { ...settings.value, enabled };
  }

  return {
    registry,
    settings,
    packages,
    issues,
    selectedPackage,
    loading,
    error,
    applySnapshot,
    initialize,
    refresh,
    importPackage,
    selectPackage,
    removePackage,
    updateSettings,
    refreshSettings,
    updatePlaybackConfig,
    setEnabled,
  };
});
