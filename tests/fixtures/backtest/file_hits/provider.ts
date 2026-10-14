/**
 * A GadgetSearchProvider provides results for one folder.
 */
export interface GadgetSearchProvider {
    provide(query: string): string[];
}

export class OldGadgetSearchProviderAdapter implements GadgetSearchProvider {
    constructor(private provider: GadgetSearchProvider) { }
    provide(query: string): string[] { return this.provider.provide(query); }
}

export function toGadgetSearchProviderOptions(folders: string[]) {
    return folders.map(folder => ({ folder }));
}
