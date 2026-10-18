use anchor_lang::prelude::*;
use deadroll_types::{seeds, ProgramIdx};

use super::set_programs::EditConfig;
use crate::{
    errors::RegistryError,
    events::SpriteBooksSet,
    state::{
        pull::{pull, KIND_SPRITE_BOOK, SPRITE_BOOK_DISCRIMINATOR},
        SPRITE_BOOKS_MAX,
    },
};

/// Authority points `GameConfig.sprite_books` at 1..=4 finalized `SpriteBook`s, given in lookup
/// order as remaining accounts (`docs/STORAGE_AND_DELIVERY.md` 2). The other slots are cleared.
pub fn handler(ctx: Context<EditConfig>) -> Result<()> {
    let render = ctx.accounts.config.core.programs[ProgramIdx::Render as usize];
    let books = ctx.remaining_accounts;
    let list = book_list(books.iter().map(|b| b.key()))?;
    for b in books {
        pull(
            b,
            &render,
            seeds::SPRITES,
            SPRITE_BOOK_DISCRIMINATOR,
            KIND_SPRITE_BOOK,
        )?;
    }
    ctx.accounts.config.sprite_books = list;
    emit!(SpriteBooksSet { sprite_books: list });
    Ok(())
}

/// 1..=4 distinct addresses, then empty slots.
pub fn book_list(keys: impl Iterator<Item = Pubkey>) -> Result<[Pubkey; SPRITE_BOOKS_MAX]> {
    let mut list = [Pubkey::default(); SPRITE_BOOKS_MAX];
    let mut n = 0;
    for k in keys {
        require!(n < SPRITE_BOOKS_MAX, RegistryError::SpriteBookCount);
        require!(!list[..n].contains(&k), RegistryError::DuplicateSpriteBook);
        list[n] = k;
        n += 1;
    }
    require!(n > 0, RegistryError::SpriteBookCount);
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_in_order_then_empty() {
        let (a, b) = (Pubkey::new_unique(), Pubkey::new_unique());
        let list = book_list([a, b].into_iter()).unwrap();
        assert_eq!(list, [a, b, Pubkey::default(), Pubkey::default()]);
        let four: Vec<Pubkey> = (0..4).map(|_| Pubkey::new_unique()).collect();
        assert_eq!(book_list(four.clone().into_iter()).unwrap().to_vec(), four);
    }

    #[test]
    fn rejects_none_too_many_and_duplicates() {
        let count: Error = RegistryError::SpriteBookCount.into();
        assert_eq!(book_list(core::iter::empty()).unwrap_err(), count);
        let five = (0..5).map(|_| Pubkey::new_unique());
        assert_eq!(book_list(five).unwrap_err(), count);
        let a = Pubkey::new_unique();
        assert_eq!(
            book_list([a, a].into_iter()).unwrap_err(),
            RegistryError::DuplicateSpriteBook.into()
        );
    }
}
