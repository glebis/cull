<script lang="ts">
	import type { Slide, ManifestDefaults, ExportTarget } from '$lib/export-types';

	let { slide, defaults, target, imageSrc }: {
		slide: Slide;
		defaults: ManifestDefaults;
		target: ExportTarget;
		imageSrc: string;
	} = $props();

	const scrimDirection = $derived(slide.overlay.scrim.direction.replace(/-/g, ' '));
	const scrimGradient = $derived(`linear-gradient(${scrimDirection}, ${slide.overlay.scrim.from}, ${slide.overlay.scrim.to})`);

	const objectPosition = $derived(slide.image.focal_point
		? `${slide.image.focal_point.x * 100}% ${slide.image.focal_point.y * 100}%`
		: '50% 50%');

	const positionClass = $derived(slide.overlay.position || 'bottom-left');
</script>

<div
	class="bleed-slide"
	style:width="{target.width}px"
	style:height="{target.height}px"
>
	<img
		src={imageSrc}
		alt={slide.metadata.alt}
		class="cover-image"
		style:object-position={objectPosition}
	/>
	<div class="scrim" style:background={scrimGradient}></div>
	<div
		class="text-overlay {positionClass}"
		style:padding-top="{defaults.safe_area.top}px"
		style:padding-right="{defaults.safe_area.right}px"
		style:padding-bottom="{defaults.safe_area.bottom}px"
		style:padding-left="{defaults.safe_area.left}px"
		style:color={slide.overlay.text_color}
	>
		{#if slide.text.headline}
			<h1 class="headline">{slide.text.headline}</h1>
		{/if}
		{#if slide.text.body}
			<p class="body">{slide.text.body}</p>
		{/if}
		{#if slide.text.caption}
			<span class="caption">{slide.text.caption}</span>
		{/if}
	</div>
</div>

<style>
	.bleed-slide {
		position: relative;
		overflow: hidden;
		background: #000;
	}

	.cover-image {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.scrim {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}

	.text-overlay {
		position: absolute;
		inset: 0;
		display: flex;
		flex-direction: column;
		box-sizing: border-box;
	}

	.text-overlay.bottom-left {
		justify-content: flex-end;
		align-items: flex-start;
	}

	.text-overlay.bottom-right {
		justify-content: flex-end;
		align-items: flex-end;
		text-align: right;
	}

	.text-overlay.top-left {
		justify-content: flex-start;
		align-items: flex-start;
	}

	.text-overlay.top-right {
		justify-content: flex-start;
		align-items: flex-end;
		text-align: right;
	}

	.text-overlay.center {
		justify-content: center;
		align-items: center;
		text-align: center;
	}

	.headline {
		font-family: var(--font-serif);
		font-size: 48px;
		font-weight: 700;
		line-height: 1.15;
		margin: 0 0 12px 0;
	}

	.body {
		font-family: var(--font-serif);
		font-size: 24px;
		font-weight: 400;
		line-height: 1.4;
		margin: 0 0 16px 0;
		opacity: 0.9;
	}

	.caption {
		font-family: var(--font);
		font-size: 14px;
		font-weight: 400;
		opacity: 0.7;
		letter-spacing: 0.02em;
	}
</style>
