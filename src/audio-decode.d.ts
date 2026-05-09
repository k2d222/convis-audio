declare module "@audio/decode-wav" {
  export interface AudioData {
    channelData: Float32Array[];
    sampleRate: number;
  }

  export default function decode(buf: ArrayBuffer | Uint8Array): Promise<AudioData>;
}
