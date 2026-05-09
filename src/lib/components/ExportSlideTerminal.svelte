<script lang="ts">
	import type { Slide, ManifestDefaults, ExportTarget } from '$lib/export-types';

	let { slide, defaults, target, imageSrc }: { slide: Slide; defaults: ManifestDefaults; target: ExportTarget; imageSrc: string } = $props();

	let objectPosition = $derived(
		slide.image.focal_point
			? `${slide.image.focal_point.x * 100}% ${slide.image.focal_point.y * 100}%`
			: '50% 50%'
	);
</script>

<div
	class="terminal-slide"
	style:width="{target.width}px"
	style:height="{target.height}px"
	style:background={defaults.colors.background || '#08080c'}
>
	<div class="accent-bar" style:background={defaults.colors.accent || '#9ece6a'}></div>
	<div
		class="content"
		style:padding-top="{defaults.safe_area.top}px"
		style:padding-right="{defaults.safe_area.right}px"
		style:padding-bottom="{defaults.safe_area.bottom}px"
		style:padding-left="{defaults.safe_area.left + 16}px"
	>
		<div class="text-col">
			{#if slide.text.headline}
				<span class="label">// headline</span>
				<h1 style:color={defaults.colors.foreground || '#e0e0e0'}>{slide.text.headline}</h1>
			{/if}
			{#if slide.text.body}
				<p class="body" style:color={defaults.colors.foreground || '#e0e0e0'}>{slide.text.body}</p>
			{/if}
			{#if slide.text.caption}
				<span class="caption">&gt; {slide.text.caption}</span>
			{/if}
		</div>
		<div class="image-col">
			<img src={imageSrc} alt={slide.metadata.alt} style:object-position={objectPosition} />
		</div>
	</div>
</div>

<style>
	.terminal-slide {
		position: relative;
		overflow: hidden;
		font-family: var(--font, 'JetBrains Mono', monospace);
	}

	.accent-bar {
		position: absolute;
		top: 0;
		left: 0;
		width: 4px;
		height: 100%;
	}

	.content {
		display: flex;
		gap: 24px;
		height: 100%;
		box-sizing: border-box;
	}

	.text-col {
		flex: 0 0 60%;
		display: flex;
		flex-direction: column;
		justify-content: center;
	}

	.label {
		font-size: 14px;
		color: #7a7fa0;
		margin-bottom: 8px;
	}

	h1 {
		font-size: 40px;
		font-weight: 700;
		line-height: 1.15;
		margin: 0 0 24px 0;
	}

	.body {
		font-size: 20px;
		font-weight: 400;
		line-height: 1.6;
		opacity: 0.8;
		margin: 0;
	}

	.caption {
		font-size: 14px;
		color: #7a7fa0;
		margin-top: auto;
		padding-top: 24px;
	}

	.image-col {
		flex: 0 0 40%;
		display: flex;
		align-items: center;
	}

	.image-col img {
		width: 100%;
		height: 100%;
		max-height: 80%;
		object-fit: cover;
		border-radius: 4px;
		border: 1px solid #1a1a2e;
	}
</style>
