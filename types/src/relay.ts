import { Client } from './client';

export namespace Relay {
  export interface Data {
    lastSignalTimestamp: number;
  }

  export const SAMPLE: Data = {
    lastSignalTimestamp: 0,
  };

  export const createRelayRequest = ({
    uid,
  }: {
    uid: string;
  }): Client.DeviceRequest['device'] => {
    return {
      uid,
      gpio: 1,
      action: 'trigger',
    };
  };
}
