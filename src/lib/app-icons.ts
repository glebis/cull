export type AppIconVariantId = 'primary' | 'red' | 'blue' | 'dark' | 'yellow';

export interface AppIconVariant {
    id: AppIconVariantId;
    label: string;
    description: string;
    asset: string;
}

export const DEFAULT_APP_ICON_VARIANT: AppIconVariantId = 'dark';

export const APP_ICON_VARIANTS: AppIconVariant[] = [
    {
        id: 'dark',
        label: 'Dark Mono',
        description: 'Black / off-white',
        asset: '/icon-variants/cull-dark.png',
    },
    {
        id: 'primary',
        label: 'Primary Mono',
        description: 'Off-white / black',
        asset: '/icon-variants/cull-primary.png',
    },
    {
        id: 'red',
        label: 'Signal Red',
        description: 'Red / off-white',
        asset: '/icon-variants/cull-red.png',
    },
    {
        id: 'blue',
        label: 'Bauhaus Blue',
        description: 'Blue / off-white',
        asset: '/icon-variants/cull-blue.png',
    },
    {
        id: 'yellow',
        label: 'Archive Yellow',
        description: 'Yellow / black',
        asset: '/icon-variants/cull-yellow.png',
    },
];

export function normalizeAppIconVariant(value: string | null): AppIconVariantId {
    return APP_ICON_VARIANTS.some(variant => variant.id === value)
        ? value as AppIconVariantId
        : DEFAULT_APP_ICON_VARIANT;
}
