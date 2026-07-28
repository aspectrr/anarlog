import { Trans } from "@lingui/react/macro";

import { ConfigureProviders } from "./configure";
import { SttSettingsProvider } from "./context";
import { DiarizeModelSection } from "./diarize-section";
import { SelectProviderAndModel } from "./select";

import { SettingsPageTitle } from "~/settings/page-title";

export function STT() {
  return (
    <SttSettingsProvider>
      <div className="flex flex-col gap-6">
        <SettingsPageTitle title={<Trans>Transcription</Trans>} />
        <SelectProviderAndModel />
        <DiarizeModelSection />
        <ConfigureProviders />
      </div>
    </SttSettingsProvider>
  );
}
