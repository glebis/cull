import "./styles.css";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `
  <section class="hero">
    <p class="eyebrow">Cull</p>
    <h1>A local-first image culling app for people and agents.</h1>
    <p class="lede">Review, rate, accept, reject, compare, and export image sets without handing your library to a cloud service.</p>
    <form class="signup-form" data-signup-form>
      <label for="email">Get the open-source launch update and early builds.</label>
      <div class="signup-row">
        <input id="email" name="email" type="email" autocomplete="email" placeholder="you@example.com" required />
        <button type="submit">Request access</button>
      </div>
      <p class="form-status" data-form-status>Confirmed opt-in. No list until you click the email.</p>
    </form>
  </section>
`;
