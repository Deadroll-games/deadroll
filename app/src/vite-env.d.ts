/// <reference types="vite/client" />
interface ImportMetaEnv {
  readonly VITE_DATA?: string;
  readonly VITE_RPC_URL?: string;
  readonly VITE_RPC_WS_URL?: string;
  readonly VITE_CHAIN?: string;
  readonly VITE_MOCK_VRF?: string;
}

declare module "virtual:wiki" {
  const wiki: import("../scripts/wiki").WikiData;
  export default wiki;
}
